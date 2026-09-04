/**
 * Zircon Cloudflare Worker: Zero-Cost Co-Op Session Rendezvous
 * 
 * Manages 6-character Join Codes (ZK-XXXX) backed by Cloudflare Workers KV.
 * KV Namespace binding: COOP_SESSIONS
 * Cost: $0.00 (operates within < 0.1% of Cloudflare's permanent free tier).
 */

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    const path = url.pathname;
    const method = request.method;

    // CORS Headers for secure launcher communication
    const corsHeaders = {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'GET, POST, DELETE, OPTIONS',
      'Access-Control-Allow-Headers': 'Content-Type, Authorization',
    };

    if (method === 'OPTIONS') {
      return new Response(null, { headers: corsHeaders });
    }

    if (!env.COOP_SESSIONS) {
      return new Response(
        JSON.stringify({ error: 'COOP_SESSIONS KV binding missing' }),
        { status: 500, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
      );
    }

    // POST /session — Host registers a new session
    if (method === 'POST' && path === '/session') {
      try {
        const body = await request.json();
        const { joinCode, host, gamePort, p2pPort, instanceName, mcVersion, loaderType } = body;

        if (!joinCode || !host) {
          return new Response(
            JSON.stringify({ error: 'Missing required joinCode or host' }),
            { status: 400, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
          );
        }

        const clientIp = request.headers.get('cf-connecting-ip') || '127.0.0.1';
        const effectiveHost = (!host || host === 'auto' || host === '127.0.0.1') ? clientIp : host;

        const sessionData = {
          joinCode: joinCode.toUpperCase(),
          host: effectiveHost,
          gamePort: gamePort || 25565,
          p2pPort: p2pPort || 25566,
          instanceName: instanceName || 'Hosted World',
          mcVersion: mcVersion || '1.21.1',
          loaderType: loaderType || 'fabric',
          createdAt: Date.now(),
        };

        // 2-hour TTL (7200 seconds)
        await env.COOP_SESSIONS.put(
          `session:${sessionData.joinCode}`,
          JSON.stringify(sessionData),
          { expirationTtl: 7200 }
        );

        return new Response(JSON.stringify({ ok: true, session: sessionData }), {
          status: 201,
          headers: { ...corsHeaders, 'Content-Type': 'application/json' },
        });
      } catch (err) {
        return new Response(
          JSON.stringify({ error: err.message }),
          { status: 400, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
        );
      }
    }

    // GET /session/:code — Guest resolves host rendezvous
    const matchGet = path.match(/^\/session\/([A-Za-z0-9\-]+)$/);
    if (method === 'GET' && matchGet) {
      const code = matchGet[1].toUpperCase();
      const raw = await env.COOP_SESSIONS.get(`session:${code}`);

      if (!raw) {
        return new Response(
          JSON.stringify({ error: `Session ${code} not found or expired` }),
          { status: 404, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
        );
      }

      return new Response(raw, {
        status: 200,
        headers: { ...corsHeaders, 'Content-Type': 'application/json' },
      });
    }

    // POST /session/:code/heartbeat — Host keeps session alive
    const matchHeartbeat = path.match(/^\/session\/([A-Za-z0-9\-]+)\/heartbeat$/);
    if (method === 'POST' && matchHeartbeat) {
      const code = matchHeartbeat[1].toUpperCase();
      const raw = await env.COOP_SESSIONS.get(`session:${code}`);
      if (!raw) {
        return new Response(JSON.stringify({ error: 'Session expired' }), {
          status: 404,
          headers: { ...corsHeaders, 'Content-Type': 'application/json' },
        });
      }
      // Re-put with renewed 7200s TTL
      await env.COOP_SESSIONS.put(`session:${code}`, raw, { expirationTtl: 7200 });
      return new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: { ...corsHeaders, 'Content-Type': 'application/json' },
      });
    }

    // DELETE /session/:code — Host cleanly closes session
    const matchDelete = path.match(/^\/session\/([A-Za-z0-9\-]+)$/);
    if (method === 'DELETE' && matchDelete) {
      const code = matchDelete[1].toUpperCase();
      await env.COOP_SESSIONS.delete(`session:${code}`);
      return new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: { ...corsHeaders, 'Content-Type': 'application/json' },
      });
    }

    return new Response(JSON.stringify({ error: 'Not found' }), {
      status: 404,
      headers: { ...corsHeaders, 'Content-Type': 'application/json' },
    });
  },
};
