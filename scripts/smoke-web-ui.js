// Smoke test for the pre-compiled admin SPA: loads the REAL vendored Vue
// runtime, generated render, feature modules and app.js into a jsdom window
// (with a stubbed fetch) and asserts the dashboard mounts and renders without
// throwing. This catches template-compiler/runtime wiring mistakes that a
// syntax check cannot — missing helpers, broken with(_ctx) references, or a
// render function that crashes on mount.
//
// Run from the workspace root: `node target/ui-build/smoke-web-ui.js`

const fs = require('fs');
const path = require('path');
const vm = require('vm');

// Locate the workspace root (works from scripts/ and target/ui-build/).
function workspaceRoot() {
  let dir = __dirname;
  while (!fs.existsSync(path.join(dir, 'Cargo.toml')) && path.dirname(dir) !== dir) {
    dir = path.dirname(dir);
  }
  return dir;
}
const ROOT = workspaceRoot();
const BUILD = path.join(ROOT, 'target', 'ui-build');
const { JSDOM } = require(require.resolve('jsdom', { paths: [path.join(BUILD, 'node_modules')] }));

const WEB = path.join(ROOT, 'crates', 'zircon-server', 'assets', 'web');

const dom = new JSDOM(
  '<!DOCTYPE html><html><body><div id="app" class="flex w-full h-full"></div></body></html>',
  {
    url: 'http://localhost/',
    pretendToBeVisual: true,
    runScripts: 'outside-only',
  },
);
dom.window.addEventListener('error', (e) => console.error('WINDOW ERROR:', e.message));
dom.window.addEventListener('unhandledrejection', (e) =>
  console.error('UNHANDLED REJECTION:', String(e.reason)),
);

const context = dom.getInternalVMContext();
context.Response = global.Response; // jsdom windows have no fetch/Response

// Stub the network: boot fires /api/auth/me, /api/instances, /api/stats etc.
// Return empty success payloads so rendering proceeds; log the calls so we can
// assert the boot actually reached the API.
vm.runInContext(
  `window.__calls = [];
  window.fetch = async (u) => {
    window.__calls.push(String(u));
    if (String(u).endsWith('/api/auth/me')) return new Response(JSON.stringify({ username: 'admin', icon: 'emerald' }), { status: 200 });
    return new Response(JSON.stringify({ instances: [], stats: {}, serverProperties: {}, shaderpacks: [], resourcepacks: [] }), { status: 200 });
  };
  window.alert = () => {};`,
  context,
);

// Seed a persisted session so boot reaches the dashboard (restoreSession
// validates it via the stubbed /api/auth/me).
vm.runInContext(`localStorage.setItem('zircon.adminToken', 'fake-jwt');`, context);

function loadScript(file) {
  vm.runInContext(fs.readFileSync(path.join(WEB, file), 'utf8'), context);
}

// Order matches index.html: runtime, feature modules, render, app.
for (const file of [
  'js/vue.runtime.global.prod.js',
  'js/core.js',
  'js/auth.js',
  'js/instances.js',
  'js/settings.js',
  'js/mods.js',
  'js/packs.js',
  'js/players.js',
  'js/backups.js',
  'js/console.js',
  'js/render.js',
  'app.js',
]) {
  loadScript(file);
}

// Boot is async: restoreSession awaits the mock /api/auth/me, flips
// authenticated, then loadInstances() runs. The initial render is a 7-byte
// "SESSION RESTORE" placeholder, so wait until the session call has fired AND
// real dashboard markup exists before asserting.
new Promise((resolve, reject) => {
  const timer = setInterval(() => {
    try {
      const appEl = dom.window.document.getElementById('app');
      const html = appEl ? appEl.innerHTML : '';
      const booted = (dom.window.__calls || []).some((c) => c.endsWith('/api/auth/me'));
      if (!booted || html.length < 2000) return;
      clearInterval(timer);
      resolve(html);
    } catch (e) {
      clearInterval(timer);
      reject(e);
    }
  }, 50);
  setTimeout(() => {
    clearInterval(timer);
    reject(new Error('timed out waiting for the dashboard to render'));
  }, 8000);
})
  .then((html) => {
    const calls = dom.window.__calls;
    if (!calls.some((c) => c.endsWith('/api/auth/me'))) {
      throw new Error(`boot never validated the session; calls: ${JSON.stringify(calls)}`);
    }
    if (html.length < 2000) {
      throw new Error(`dashboard rendered too little: ${html.length} bytes`);
    }
    for (const marker of ['Mods', 'zircon-title.svg', 'LEFT SIDEBAR']) {
      if (!html.includes(marker)) {
        throw new Error(`dashboard missing expected marker '${marker}': ${html.slice(0, 400)}`);
      }
    }
    console.log(`OK: dashboard mounted, rendered ${html.length} bytes (sidebar + Mods view)`);
    dom.window.close();
    process.exit(0);
  })
  .catch((e) => {
    console.error('FAIL:', e.message || e);
    dom.window.close();
    process.exit(1);
  });
