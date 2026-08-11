# Agent Execution Plan: Web GUI Enhancements & Modpack Support

## Overview
This plan guides an automated AI agent through implementing five major Web GUI enhancements for the **Zircon (McManager)** server administration panel:
1. **Interactive Mod Installation State**: Automatically remove or hide mods from search results once installed.
2. **Installation Throbber/Spinner**: Display a clear loading spinner and disabled state during mod downloads.
3. **Server Restart/Reset Button**: Add a "Restart Server" action adjacent to the Start/Stop buttons.
4. **Modpack Download & Installation**: Add Modrinth modpack search, `.mrpack` parsing, and automated multi-mod installation.
5. **Zircon Recommended Mods Section**: Add a curated section for team-recommended essential mods with 1-click installation.

---

## Phase 1: Git Workflow & Environment Setup

### Instructions for Agent:
Execute the following git commands before modifying any files:

```bash
# 1. Ensure you are on main and up to date
git checkout main
git pull origin main

# 2. Create a feature branch for these changes
git checkout -b feature/web-gui-modpacks-and-enhancements
```

---

## Phase 2: Backend API Additions (Server Restart & Modpack Engine)

### Files to Modify / Create:
1. `main/java/com/mcmanager/core/api/ModrinthApiClient.java`
2. `main/java/com/mcmanager/server/service/ModManagementService.java`
3. `main/java/com/mcmanager/server/web/controller/InstanceController.java`
4. `main/java/com/mcmanager/server/web/JavalinApp.java`

---

### Step 2.1: Enhance `ModrinthApiClient.java` for Modpacks & Filter Types

Add support for filtering search results by `project_type` (`mod` vs `modpack`) and downloading project `.mrpack` archives.

#### Pseudo-code Changes:
```java
// In ModrinthApiClient.java

/**
 * Searches Modrinth with optional project_type filter ("mod" or "modpack").
 */
public List<ModrinthSearchHit> searchProjects(String query, String mcVersion, String loaderType, String projectType) 
        throws IOException, InterruptedException {
    StringBuilder url = new StringBuilder(BASE_URL).append("/search?query=")
            .append(urlEncode(query == null ? "" : query));

    List<String> facetGroups = new ArrayList<>();
    if (mcVersion != null && !mcVersion.isBlank()) {
        facetGroups.add("[\"versions:" + mcVersion + "\"]");
    }
    if (loaderType != null && !loaderType.isBlank()) {
        facetGroups.add("[\"categories:" + loaderType + "\"]");
    }
    if (projectType != null && !projectType.isBlank()) {
        facetGroups.add("[\"project_type:" + projectType + "\"]");
    }

    if (!facetGroups.isEmpty()) {
        url.append("&facets=").append(urlEncode("[" + String.join(",", facetGroups) + "]"));
    }
    url.append("&limit=25");

    // Execute HTTP GET and return List<ModrinthSearchHit>
}
```

---

### Step 2.2: Implement Modpack Downloader & Installer in `ModManagementService.java`

Modrinth modpacks are `.mrpack` files (ZIP format containing `modrinth.index.json`). We need logic to download the `.mrpack`, parse `modrinth.index.json`, download all listed mod JARs into the instance's `mods/` directory, and register them in `bom.json`.

#### Key Implementation Details:
- Inspect `modrinth.index.json`:
  ```json
  {
    "formatVersion": 1,
    "game": "minecraft",
    "files": [
      {
        "path": "mods/sodium-fabric-0.5.8.jar",
        "hashes": { "sha1": "..." },
        "downloads": ["https://cdn.modrinth.com/data/.../sodium.jar"]
      }
    ]
  }
  ```
- Extract each entry in `files` whose `path` starts with `mods/`.
- Download each file to `mods/<filename>` and add an entry to `BillOfMaterials`.

#### Pseudo-code Changes:
```java
// In ModManagementService.java

public synchronized Map<String, Object> installModrinthModpack(String projectId, String versionId) 
        throws IOException, InterruptedException {
    // 1. Fetch Modrinth Version details to get .mrpack file URL
    List<ModrinthApiClient.ModrinthVersion> versions = modrinth.listProjectVersions(projectId, null, null);
    ModrinthApiClient.ModrinthVersion version = versions.stream()
            .filter(v -> versionId == null || versionId.equals(v.id))
            .findFirst()
            .orElseThrow(() -> new IOException("Modpack version not found"));

    ModrinthApiClient.ModrinthFile primaryFile = version.primaryFile();
    if (primaryFile == null || !primaryFile.filename.endsWith(".mrpack")) {
        throw new IOException("Selected version does not contain a valid .mrpack file");
    }

    // 2. Download .mrpack to temporary file
    Path tempMrpack = Files.createTempFile("modpack-", ".mrpack");
    try (InputStream in = new URI(primaryFile.url).toURL().openStream()) {
        Files.copy(in, tempMrpack, StandardCopyOption.REPLACE_EXISTING);
    }

    // 3. Unzip and read modrinth.index.json
    int installedCount = 0;
    try (ZipFile zip = new ZipFile(tempMrpack.toFile())) {
        ZipEntry indexEntry = zip.getEntry("modrinth.index.json");
        if (indexEntry == null) {
            throw new IOException("Invalid .mrpack: missing modrinth.index.json");
        }

        JsonObject indexJson = JsonParser.parseReader(
                new InputStreamReader(zip.getInputStream(indexEntry), StandardCharsets.UTF_8)).getAsJsonObject();

        JsonArray files = indexJson.getAsJsonArray("files");
        for (JsonElement element : files) {
            JsonObject fileObj = element.getAsJsonObject();
            String path = fileObj.get("path").getAsString();
            
            // Only process mods
            if (path.startsWith("mods/")) {
                String filename = path.substring("mods/".length());
                JsonArray downloads = fileObj.getAsJsonArray("downloads");
                if (downloads.size() > 0) {
                    String downloadUrl = downloads.get(0).getAsString();
                    installFromUrl(downloadUrl, filename, ORIGIN_MODRINTH);
                    installedCount++;
                }
            }
        }
    } finally {
        Files.deleteIfExists(tempMrpack);
    }

    Map<String, Object> result = new HashMap<>();
    result.put("installedCount", installedCount);
    result.put("message", "Successfully installed modpack (" + installedCount + " mods)");
    return result;
}
```

---

### Step 2.3: Add Server Restart & Modpack Endpoints in `InstanceController.java` & `JavalinApp.java`

#### 1. Add `restartInstance` method in `InstanceController.java`:
```java
/** POST /api/instances/{id}/restart */
public void restartInstance(Context ctx) {
    String id = ctx.pathParam("id");
    try {
        instanceManager.getInstance(id); // Ensure instance exists
        instanceManager.stopInstance(id);
        
        // Spawn background thread to wait briefly and restart
        CompletableFuture.runAsync(() -> {
            try {
                Thread.sleep(1500); // Allow OS process cleanup
                instanceManager.startInstance(id);
            } catch (Exception e) {
                log.error("Failed to restart instance {}", id, e);
            }
        });
        
        ctx.json(Map.of("ok", true, "message", "Server is restarting..."));
    } catch (IllegalArgumentException e) {
        ctx.status(404).result(e.getMessage());
    } catch (Exception e) {
        ctx.status(500).result("Restart failed: " + e.getMessage());
    }
}
```

#### 2. Add `installModpack` route handler in `InstanceController.java`:
```java
/** POST /api/instances/{id}/modpacks/install */
public void installModpack(Context ctx) {
    InstallRequest body = ctx.bodyAsClass(InstallRequest.class);
    if (body == null || body.projectId == null) {
        ctx.status(400).result("projectId is required");
        return;
    }

    try {
        ModManagementService mods = modsFor(ctx.pathParam("id"));
        Map<String, Object> result = mods.installModrinthModpack(body.projectId, body.versionId);
        ctx.status(201).json(result);
    } catch (Exception e) {
        ctx.status(500).result("Modpack installation failed: " + e.getMessage());
    }
}
```

#### 3. Register routes in `JavalinApp.java`:
```java
// In JavalinApp.java start() method
app.post("/api/instances/{id}/restart", instanceController::restartInstance);
app.post("/api/instances/{id}/modpacks/install", instanceController::installModpack);
```

---

## Phase 3: Frontend Implementation (`index.html`)

All Web UI enhancements will be implemented in `main/resources/web/index.html` using Vue 3 and Tailwind CSS.

---

### Step 3.1: Server Restart / Reset Button

#### Requirements:
- Place a **Restart** button next to the **Start** and **Stop** buttons in both the sidebar instance list and top header controls.
- Style with a distinctive amber/orange theme or refresh icon `🔄`.

#### Code Changes (`index.html`):

1. **In Sidebar Server Cards:**
```html
<div class="flex gap-1.5 mt-2">
    <button v-if="!inst.running" @click.stop="startInstance(inst)"
            class="flex-1 bg-emerald-600 hover:bg-emerald-500 text-xs font-semibold py-1.5 rounded-lg transition">Start</button>
    <template v-else>
        <button @click.stop="restartInstance(inst)"
                class="bg-amber-600/80 hover:bg-amber-500 text-xs font-semibold px-2.5 py-1.5 rounded-lg transition flex items-center justify-center gap-1"
                title="Restart Server">🔄 Restart</button>
        <button @click.stop="stopInstance(inst)"
                class="flex-1 bg-red-600/80 hover:bg-red-500 text-xs font-semibold py-1.5 rounded-lg transition">Stop</button>
    </template>
</div>
```

2. **In Vue Methods (`createApp`):**
```javascript
async restartInstance(inst) {
    inst = inst || this.selectedInstance;
    if (!inst) return;
    try {
        await this.api(`/api/instances/${inst.id}/restart`, { method: 'POST' });
        alert(`Server "${inst.name}" is restarting...`);
        setTimeout(() => this.loadInstances(), 2000);
    } catch (e) {
        alert('Restart failed: ' + e.message);
    }
}
```

---

### Step 3.2: Installation Spinner & Auto-Remove/Hide Installed Mods

#### Requirements:
- Track `installingMods` reactive object in Vue (`installingMods[projectId] = true`).
- Show an animated SVG loading spinner on the button while installing.
- Disable the button to prevent duplicate clicks.
- Upon completion, filter out the installed mod from `searchResults` (or flag as installed) and refresh `installedMods`.

#### Code Changes (`index.html`):

1. **Vue Reactive State:**
```javascript
data() {
    return {
        // ... existing state
        installingMods: {}, // Tracks { [projectId]: boolean }
        // ...
    }
}
```

2. **Updated `installMod` Method:**
```javascript
async installMod(hit) {
    const id = hit.projectId;
    this.installingMods[id] = true;
    try {
        const loader = this.selectedInstance.modLoader.type === 'vanilla' ? '' : this.selectedInstance.modLoader.type;
        const q = new URLSearchParams({ projectId: id, mcVersion: this.selectedInstance.minecraftVersion, loader });
        const versions = await this.api(`/api/instances/${this.selectedInstance.id}/mods/modrinth/versions?${q}`);
        const chosen = (versions.versions || [])[0];
        
        if (!chosen) { 
            alert('No compatible version found for ' + this.selectedInstance.minecraftVersion); 
            return; 
        }

        await this.api(`/api/instances/${this.selectedInstance.id}/mods/install`, {
            method: 'POST',
            body: JSON.stringify({ origin: 'modrinth', projectId: id, versionId: chosen.id })
        });

        // 1. Remove mod from search results view immediately
        this.searchResults = this.searchResults.filter(r => r.projectId !== id);

        // 2. Reload installed mods list
        await this.loadMods();
    } catch (e) {
        alert('Install failed: ' + e.message);
    } finally {
        delete this.installingMods[id];
    }
}
```

3. **Search Result Item Template with Loading Spinner:**
```html
<div v-for="hit in searchResults" :key="hit.projectId" class="bg-slate-800/60 border border-slate-700/40 p-3 rounded-lg flex gap-3">
    <img :src="hit.iconUrl || defaultModIcon" class="w-10 h-10 rounded object-cover shrink-0">
    <div class="flex-1 min-w-0">
        <div class="flex items-center justify-between">
            <p class="font-semibold text-sm truncate">{{ hit.title }}</p>
            <button @click="installMod(hit)" 
                    :disabled="installingMods[hit.projectId]"
                    class="bg-emerald-600/20 text-emerald-300 border border-emerald-500/30 hover:bg-emerald-600/30 text-xs px-3 py-1.5 rounded-md font-medium shrink-0 flex items-center gap-1.5 transition disabled:opacity-50 disabled:cursor-not-allowed">
                <!-- Spinner SVG when installing -->
                <svg v-if="installingMods[hit.projectId]" class="animate-spin h-3.5 w-3.5 text-emerald-400" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                </svg>
                <span>{{ installingMods[hit.projectId] ? 'Downloading...' : 'Install' }}</span>
            </button>
        </div>
        <p class="text-xs text-slate-400 line-clamp-2 mt-1">{{ hit.description }}</p>
        <p class="text-[10px] text-slate-500 mt-1">by {{ hit.author }}</p>
    </div>
</div>
```

---

### Step 3.3: Zircon Recommended Mods Section

Add a curated list of essential performance and utility mods (e.g., Sodium, Lithium, FerriteCore, Iris, Cloth Config, AppleSkin) tailored for the active mod loader.

#### Curated List Data Structure:
```javascript
recommendedModsList: [
    { projectId: 'AANawA23', title: 'Sodium', slug: 'sodium', description: 'Modern rendering engine for Minecraft that greatly improves frame rates.', iconUrl: 'https://cdn.modrinth.com/data/AANawA23/icon.png', loader: 'fabric' },
    { projectId: 'gvA23421', title: 'Lithium', slug: 'lithium', description: 'General-purpose optimization mod for physics, chunk loading, and entity ticking.', iconUrl: 'https://cdn.modrinth.com/data/gvA23421/icon.png', loader: 'fabric' },
    { projectId: 'uXXiz334', title: 'FerriteCore', slug: 'ferrite-core', description: 'Memory usage optimizations for Minecraft.', iconUrl: 'https://cdn.modrinth.com/data/uXXiz334/icon.png', loader: 'both' },
    { projectId: 'YL57zq9U', title: 'Iris Shaders', slug: 'iris', description: 'A modern shaders mod for Minecraft built to be compatible with Sodium.', iconUrl: 'https://cdn.modrinth.com/data/YL57zq9U/icon.png', loader: 'fabric' },
    { projectId: '9s6D2G2q', title: 'AppleSkin', slug: 'appleskin', description: 'Adds food value information to tooltips and HUD.', iconUrl: 'https://cdn.modrinth.com/data/9s6D2G2q/icon.png', loader: 'both' }
]
```

#### UI Layout Component:
Add a **"Recommended Mods"** accordion or sub-section above or next to installed mods.

```html
<!-- Recommended Mods Accordion / Card -->
<div class="bg-slate-900 border border-slate-800 rounded-xl p-4 mb-4">
    <div class="flex items-center justify-between mb-3">
        <div class="flex items-center gap-2">
            <span class="text-base">⚡</span>
            <h3 class="font-bold text-sm text-slate-200">Zircon Team Recommended Mods</h3>
        </div>
        <span class="text-[10px] bg-emerald-500/10 text-emerald-400 px-2 py-0.5 rounded font-mono">Curated</span>
    </div>
    <div class="grid grid-cols-1 gap-2.5 max-h-48 overflow-y-auto pr-1">
        <div v-for="rec in filteredRecommendedMods" :key="rec.projectId" 
             class="bg-slate-800/40 border border-slate-700/30 p-2.5 rounded-lg flex items-center justify-between gap-3">
            <div class="flex items-center gap-2.5 min-w-0">
                <img :src="rec.iconUrl" class="w-8 h-8 rounded object-cover shrink-0">
                <div class="min-w-0">
                    <p class="font-semibold text-xs text-slate-200 truncate">{{ rec.title }}</p>
                    <p class="text-[11px] text-slate-400 truncate">{{ rec.description }}</p>
                </div>
            </div>
            
            <button v-if="!isModInstalled(rec.slug)" 
                    @click="installMod({ projectId: rec.projectId, title: rec.title })"
                    :disabled="installingMods[rec.projectId]"
                    class="bg-emerald-600/20 text-emerald-300 border border-emerald-500/30 hover:bg-emerald-600/30 text-xs px-2.5 py-1 rounded font-medium shrink-0 flex items-center gap-1">
                <svg v-if="installingMods[rec.projectId]" class="animate-spin h-3 w-3 text-emerald-400" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                </svg>
                <span>{{ installingMods[rec.projectId] ? 'Installing...' : '+ Add' }}</span>
            </button>
            <span v-else class="text-[11px] text-emerald-400 font-semibold px-2 py-0.5 bg-emerald-500/10 rounded">✓ Installed</span>
        </div>
    </div>
</div>
```

---

### Step 3.4: Modpack Search & 1-Click Install UI

In the "Find & Install Mods" panel, add a Segmented Control/Tab to toggle between searching **Mods** and **Modpacks**.

#### UI Controls:
```html
<div class="flex items-center justify-between mb-3">
    <h3 class="font-bold text-sm">Find & Install</h3>
    <!-- Search Type Toggle -->
    <div class="flex bg-slate-800 p-0.5 rounded-lg border border-slate-700 text-xs font-medium">
        <button @click="searchType = 'mod'; searchMods()" 
                :class="searchType === 'mod' ? 'bg-emerald-600 text-white shadow' : 'text-slate-400 hover:text-slate-200'"
                class="px-2.5 py-1 rounded-md transition">Mods</button>
        <button @click="searchType = 'modpack'; searchMods()" 
                :class="searchType === 'modpack' ? 'bg-emerald-600 text-white shadow' : 'text-slate-400 hover:text-slate-200'"
                class="px-2.5 py-1 rounded-md transition">Modpacks 📦</button>
    </div>
</div>
```

#### Modpack Installation Handler:
```javascript
async installModpack(hit) {
    const id = hit.projectId;
    this.installingMods[id] = true;
    try {
        const res = await this.api(`/api/instances/${this.selectedInstance.id}/modpacks/install`, {
            method: 'POST',
            body: JSON.stringify({ projectId: id })
        });
        alert(res.message || 'Modpack installed successfully!');
        await this.loadMods();
    } catch (e) {
        alert('Modpack install failed: ' + e.message);
    } finally {
        delete this.installingMods[id];
    }
}
```

---

## Phase 4: Verification & Testing Checklist

Before committing and merging back to `main`, verify the following functionality:

1. **Gradle Build Check**:
   ```bash
   ./gradlew build
   ```
2. **Server Launch**:
   Run the server wrapper and log into the web admin panel (`http://localhost:25565` or configured port).
3. **Interactive Test Steps**:
   - [ ] Click **Search** in the Mods tab and click **Install** on a mod.
   - [ ] Verify that the **Install** button changes to "Downloading..." with an animated spinning throbber.
   - [ ] Verify that the installed mod vanishes from the search results once download finishes and appears in **Installed Mods**.
   - [ ] Test the **Restart Server** button. Verify that the server stops, waits, and boots back up automatically.
   - [ ] Switch search type to **Modpacks 📦**, search for a modpack (e.g. "Fabulously Optimized"), and click Install. Verify all included mods download into `mods/`.
   - [ ] Verify the **Recommended Mods** section renders and allows 1-click installation.

---

## Phase 5: Git Merge & Cleanup

Execute the following git commands once all tests pass:

```bash
# 1. Stage and commit all changes
git add .
git commit -m "feat(web-gui): add modpack support, installation spinners, restart button, and recommended mods"

# 2. Checkout main and merge feature branch
git checkout main
git merge feature/web-gui-modpacks-and-enhancements

# 3. Clean up local feature branch
git branch -d feature/web-gui-modpacks-and-enhancements
```
