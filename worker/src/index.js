export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const path = url.pathname;

    const corsHeaders = {
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
      "Access-Control-Allow-Headers": "Content-Type",
    };

    if (request.method === "OPTIONS") {
      return new Response(null, { headers: corsHeaders });
    }

    try {
      if (request.method === "GET" && path === "/api/leaderboard") {
        return await getLeaderboard(env, corsHeaders);
      }

      const replayMatch = path.match(/^\/api\/replay\/(\d+)$/);
      if (request.method === "GET" && replayMatch) {
        const index = parseInt(replayMatch[1], 10);
        return await getReplay(env, corsHeaders, index);
      }

      if (request.method === "POST" && path === "/api/leaderboard") {
        return await submitScore(env, corsHeaders, request);
      }

      return json({ error: "Not found" }, 404, corsHeaders);
    } catch (e) {
      return json({ error: e.message }, 500, corsHeaders);
    }
  },
};

function json(data, status = 200, corsHeaders = {}) {
  return new Response(JSON.stringify(data), {
    status,
    headers: { "Content-Type": "application/json", ...corsHeaders },
  });
}

async function getLeaderboard(env, corsHeaders) {
  const raw = await env.LEADERBOARD.get("top5");
  const entries = raw ? JSON.parse(raw) : [];
  return json(entries, 200, corsHeaders);
}

async function getReplay(env, corsHeaders, index) {
  const raw = await env.LEADERBOARD.get("top5");
  const entries = raw ? JSON.parse(raw) : [];

  if (index < 0 || index >= entries.length) {
    return json({ error: "Index out of range" }, 404, corsHeaders);
  }

  const entry = entries[index];
  const replayRaw = await env.LEADERBOARD.get(`replay:${entry.id}`);
  if (!replayRaw) {
    return json({ error: "Replay data not found" }, 404, corsHeaders);
  }

  const inputs = JSON.parse(replayRaw);
  return json(
    { seed: entry.seed, level: entry.level, inputs },
    200,
    corsHeaders,
  );
}

async function submitScore(env, corsHeaders, request) {
  const body = await request.json();
  const { time, name, seed, level, inputs } = body;

  if (
    typeof time !== "number" ||
    typeof name !== "string" ||
    !name.trim() ||
    typeof seed !== "string" ||
    typeof level !== "number" ||
    !Array.isArray(inputs)
  ) {
    return json({ error: "Invalid body" }, 400, corsHeaders);
  }

  const id = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;

  const raw = await env.LEADERBOARD.get("top5");
  const entries = raw ? JSON.parse(raw) : [];

  // Check if this score qualifies
  if (entries.length >= 5 && time >= entries[entries.length - 1].time) {
    return json({ ok: false }, 200, corsHeaders);
  }

  const newEntry = { time, name: name.trim().slice(0, 16), seed, level, id };
  entries.push(newEntry);
  entries.sort((a, b) => a.time - b.time);

  // Evict entries beyond top 5 and delete their replay data
  const evicted = entries.splice(5);
  for (const e of evicted) {
    await env.LEADERBOARD.delete(`replay:${e.id}`);
  }

  // Store updated top5 and replay data
  await env.LEADERBOARD.put("top5", JSON.stringify(entries));
  await env.LEADERBOARD.put(`replay:${id}`, JSON.stringify(inputs));

  const rank = entries.findIndex((e) => e.id === id) + 1;
  return json({ ok: true, rank }, 200, corsHeaders);
}
