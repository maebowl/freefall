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
        const level = url.searchParams.get("level") || "default";
        return await getLeaderboard(env, corsHeaders, level);
      }

      const replayMatch = path.match(/^\/api\/replay\/(\d+)$/);
      if (request.method === "GET" && replayMatch) {
        const index = parseInt(replayMatch[1], 10);
        const level = url.searchParams.get("level") || "default";
        return await getReplay(env, corsHeaders, index, level);
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

function leaderboardKey(level) {
  return `top5:${level}`;
}

async function getLeaderboard(env, corsHeaders, level) {
  const raw = await env.LEADERBOARD.get(leaderboardKey(level));
  const entries = raw ? JSON.parse(raw) : [];
  return json(entries, 200, corsHeaders);
}

async function getReplay(env, corsHeaders, index, level) {
  const raw = await env.LEADERBOARD.get(leaderboardKey(level));
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
    { seed: entry.seed, inputs },
    200,
    corsHeaders,
  );
}

async function submitScore(env, corsHeaders, request) {
  const body = await request.json();
  const { time, name, seed, inputs, level } = body;

  if (
    typeof time !== "number" ||
    typeof name !== "string" ||
    !name.trim() ||
    typeof seed !== "string" ||
    !Array.isArray(inputs) ||
    typeof level !== "string" ||
    !level.trim()
  ) {
    return json({ error: "Invalid body" }, 400, corsHeaders);
  }

  const key = leaderboardKey(level.trim());
  const id = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;

  const raw = await env.LEADERBOARD.get(key);
  const entries = raw ? JSON.parse(raw) : [];

  // Check if this score qualifies
  if (entries.length >= 5 && time >= entries[entries.length - 1].time) {
    return json({ ok: false }, 200, corsHeaders);
  }

  const newEntry = { time, name: name.trim().slice(0, 16), seed, id };
  entries.push(newEntry);
  entries.sort((a, b) => a.time - b.time);

  // Evict entries beyond top 5 and delete their replay data
  const evicted = entries.splice(5);
  for (const e of evicted) {
    await env.LEADERBOARD.delete(`replay:${e.id}`);
  }

  // Store updated leaderboard and replay data
  await env.LEADERBOARD.put(key, JSON.stringify(entries));
  await env.LEADERBOARD.put(`replay:${id}`, JSON.stringify(inputs));

  const rank = entries.findIndex((e) => e.id === id) + 1;
  return json({ ok: true, rank }, 200, corsHeaders);
}
