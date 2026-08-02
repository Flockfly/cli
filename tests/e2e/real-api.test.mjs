import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawn } from "node:child_process";
import { after, before, test } from "node:test";
import { pathToFileURL } from "node:url";

const routerRoot =
  process.env.FLOCKFLY_CONTEXT_ROUTER_DIR ??
  "/Users/jkim/Documents/flockfly/context-router";
const binary =
  process.env.FLOCKFLY_BIN ??
  resolve(import.meta.dirname, "../../target/debug/flockfly");

const PUBLIC_COLLECTION_ID = "coll_public";

let created;
let server;
let baseUrl;
const temporaryDirectories = [];

before(async () => {
  const apiModule = await import(
    pathToFileURL(join(routerRoot, "api/src/index.ts")).href
  );
  const serverModule = await import(
    pathToFileURL(
      join(routerRoot, "node_modules/@hono/node-server/dist/index.mjs"),
    ).href
  );
  created = await apiModule.createApp();
  await new Promise((resolveReady) => {
    server = serverModule.serve(
      { fetch: created.app.fetch, port: 0, hostname: "127.0.0.1" },
      (info) => {
        baseUrl = `http://127.0.0.1:${info.port}`;
        resolveReady();
      },
    );
  });
});

after(async () => {
  await new Promise((resolveClosed) => server.close(resolveClosed));
  await created.close();
  await Promise.all(
    temporaryDirectories.map((directory) =>
      rm(directory, { recursive: true, force: true }),
    ),
  );
});

// The production public-collection publisher allowlist only covers
// jonathan@flockfly.ai; grant test users publish access directly.
async function grantPublish(email) {
  const user = await created.sql.get(
    "SELECT id FROM users WHERE email = $1",
    [email],
  );
  await created.sql.run(
    `INSERT INTO entity_access (id, entity_type, entity_id, email, user_id, access_type, status, granted_by_user_id, created_at)
     VALUES ($1, 'collection', $2, $3, $4, 'create', 'active', $4, $5)
     ON CONFLICT (entity_type, entity_id, email, access_type) DO NOTHING`,
    [
      `acc_rust_e2e_${user.id}`,
      PUBLIC_COLLECTION_ID,
      email,
      user.id,
      new Date().toISOString(),
    ],
  );
}

async function runProcess(args, env, { approveEmail, input } = {}) {
  return new Promise((resolveRun, rejectRun) => {
    const child = spawn(binary, args, {
      env: { ...process.env, ...env },
      stdio: ["pipe", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    let approved = false;
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
      if (approveEmail && !approved) {
        const cliAuthId = stdout.match(/cliAuthId=([A-Za-z0-9_-]+)/)?.[1];
        if (cliAuthId) {
          approved = true;
          void fetch(
            `${baseUrl}/v1/auth/cli/callback?cliAuthId=${cliAuthId}&email=${encodeURIComponent(approveEmail)}`,
          ).catch(rejectRun);
        }
      }
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", rejectRun);
    child.on("close", (code) => resolveRun({ code, stdout, stderr }));
    if (input) child.stdin.end(input);
    else child.stdin.end();
  });
}

async function loginSession(email) {
  const configDir = await mkdtemp(join(tmpdir(), "flockfly-rust-e2e-"));
  temporaryDirectories.push(configDir);
  const env = {
    FLOCKFLY_API_URL: baseUrl,
    FLOCKFLY_CONFIG_DIR: configDir,
  };
  const login = await runProcess(["login"], env, { approveEmail: email });
  assert.equal(login.code, 0, login.stderr);
  const credentials = JSON.parse(
    await readFile(join(configDir, "credentials.json"), "utf8"),
  );
  return {
    run: (args, options) => runProcess(args, env, options),
    api: async (method, path, body) => {
      const response = await fetch(`${baseUrl}${path}`, {
        method,
        headers: {
          authorization: `Bearer ${credentials.token}`,
          ...(body === undefined ? {} : { "content-type": "application/json" }),
        },
        body: body === undefined ? undefined : JSON.stringify(body),
      });
      return response.json();
    },
  };
}

async function writeSkillDirectory(name, description) {
  const directory = await mkdtemp(join(tmpdir(), "flockfly-rust-e2e-skill-"));
  temporaryDirectories.push(directory);
  await writeFile(
    join(directory, "SKILL.md"),
    `---\nname: ${name}\ndescription: ${description}\n---\n# ${name}\n\nLoad references/guide.md when needed.\n`,
  );
  await mkdir(join(directory, "references"));
  await writeFile(
    join(directory, "references/guide.md"),
    `# ${name} guide\n`,
  );
  return directory;
}

test("TS E2E: covers the full router journey with visibility rules and telemetry", async () => {
  const jane = await loginSession("jane@rust-e2e.dev");
  await grantPublish("jane@rust-e2e.dev");
  const { router } = await jane.api("POST", "/v1/routers", { name: "eng-rust" });
  await jane.api("POST", `/v1/routers/${router.id}/members`, {
    email: "sam@rust-e2e.dev",
  });

  const routerSkillDir = await writeSkillDirectory(
    "incident-runbook-rust",
    "How to handle production incidents.",
  );
  const published = await jane.run([
    "publish",
    routerSkillDir,
    "--router",
    "eng-rust",
  ]);
  assert.equal(published.code, 0, published.stderr);
  const routerSkillId = published.stdout.match(/skill_\S+(?= \(version)/)?.[0];
  assert.match(routerSkillId, /^skill_/);

  const unroutedDir = await writeSkillDirectory(
    "quarterly-report-rust",
    "Quarterly reporting workflow.",
  );
  const unrouted = await jane.run(["publish", unroutedDir]);
  const unroutedSkillId = unrouted.stdout.match(/skill_\S+(?= \(version)/)?.[0];

  const sam = await loginSession("sam@rust-e2e.dev");
  const samRouters = await sam.run(["routers", "list"]);
  assert.match(samRouters.stdout, /eng-rust/);

  const search = await sam.run(["search", "handle production incidents"]);
  assert.equal(search.code, 0, search.stderr);
  assert.match(search.stdout, new RegExp(routerSkillId));
  assert.doesNotMatch(search.stdout, new RegExp(unroutedSkillId));

  const collectionList = await sam.run(["skills", "list"]);
  assert.match(collectionList.stdout, /quarterly-report-rust/);
  const deniedLoad = await sam.run(["load", unroutedSkillId]);
  assert.equal(deniedLoad.code, 1);
  assert.match(deniedLoad.stderr, /not attached/);

  const load = await sam.run(["load", routerSkillId]);
  assert.equal(load.code, 0, load.stderr);
  assert.match(load.stdout, /# incident-runbook-rust/);
  assert.doesNotMatch(load.stdout, /# incident-runbook-rust guide/);
  const refLoad = await sam.run([
    "load",
    routerSkillId,
    "references/guide.md",
  ]);
  assert.match(refLoad.stdout, /# incident-runbook-rust guide/);

  const searchEvent = await created.sql.get(
    "SELECT id FROM search_events WHERE query = $1 ORDER BY created_at DESC LIMIT 1",
    ["handle production incidents"],
  );
  const impressions = await created.sql.all(
    "SELECT * FROM search_result_impressions WHERE search_event_id = $1",
    [searchEvent.id],
  );
  assert.ok(impressions.length > 0);
  assert.equal(impressions[0].rank, 1);
  const loads = await created.sql.all(
    "SELECT * FROM load_events WHERE skill_id = $1 ORDER BY created_at ASC",
    [routerSkillId],
  );
  assert.equal(loads.length, 2);
  assert.equal(loads[0].correlated_search_event_id, searchEvent.id);

  const feedback = await sam.api(
    "POST",
    `/v1/skills/${routerSkillId}/feedback`,
    { value: "up" },
  );
  assert.deepEqual(feedback, { mine: "up", up: 1, down: 0 });
});

test("TS E2E: replaces a skill after confirmation and search returns the new version", async () => {
  const owner = await loginSession("replace@rust-e2e.dev");
  await grantPublish("replace@rust-e2e.dev");
  const versionOne = await writeSkillDirectory(
    "design-review-rust",
    "Original review checklist.",
  );
  const publishedOne = await owner.run([
    "publish",
    versionOne,
    "--router",
    "replace's router",
  ]);
  assert.equal(publishedOne.code, 0, publishedOne.stderr);

  const versionTwo = await writeSkillDirectory(
    "design-review-rust",
    "Updated review checklist with security section.",
  );
  const publishedTwo = await owner.run(["publish", versionTwo], { input: "y\n" });
  assert.equal(publishedTwo.code, 0, publishedTwo.stderr);
  assert.match(publishedTwo.stdout, /\(version 2\)/);

  const search = await owner.run(["search", "design-review-rust"]);
  assert.match(search.stdout, /Updated review checklist with security section/);
  assert.doesNotMatch(search.stdout, /Original review checklist/);

  const skillId = publishedTwo.stdout.match(/skill_\S+(?= \(version)/)?.[0];
  const load = await owner.run(["load", skillId]);
  assert.equal(load.code, 0, load.stderr);
});

test("search --load selects and renders the top real-API result without loading empty results", async () => {
  const owner = await loginSession("search-load@rust-e2e.dev");
  await grantPublish("search-load@rust-e2e.dev");
  const secondaryDirectory = await writeSkillDirectory(
    "search-load-secondary-rust",
    "A generic workflow unrelated to the quartz sentinel.",
  );
  const secondary = await owner.run([
    "publish",
    secondaryDirectory,
    "--router",
    "search-load's router",
  ]);
  assert.equal(secondary.code, 0, secondary.stderr);

  const topDirectory = await writeSkillDirectory(
    "search-load-top-rust",
    "Search load integration sentinel quartz workflow.",
  );
  const top = await owner.run([
    "publish",
    topDirectory,
    "--router",
    "search-load's router",
  ]);
  assert.equal(top.code, 0, top.stderr);
  const topSkillId = top.stdout.match(/skill_\S+(?= \(version)/)?.[0];

  const loaded = await owner.run([
    "search",
    "search load integration sentinel quartz",
    "--load",
  ]);
  assert.equal(loaded.code, 0, loaded.stderr);
  assert.match(loaded.stdout, /# search-load-top-rust/);
  assert.doesNotMatch(loaded.stdout, /search-load-secondary-rust/);
  assert.doesNotMatch(loaded.stdout, /^1\. skill_/m);

  const searchEvent = await created.sql.get(
    "SELECT id FROM search_events WHERE query = $1 ORDER BY created_at DESC LIMIT 1",
    ["search load integration sentinel quartz"],
  );
  const firstImpression = await created.sql.get(
    "SELECT skill_id, rank FROM search_result_impressions WHERE search_event_id = $1 ORDER BY rank ASC LIMIT 1",
    [searchEvent.id],
  );
  assert.equal(firstImpression.skill_id, topSkillId);
  assert.equal(firstImpression.rank, 1);
  const correlatedLoad = await created.sql.get(
    "SELECT correlated_search_event_id FROM load_events WHERE skill_id = $1 ORDER BY created_at DESC LIMIT 1",
    [topSkillId],
  );
  assert.equal(correlatedLoad.correlated_search_event_id, searchEvent.id);

  const before = await created.sql.get(
    "SELECT COUNT(*)::int AS n FROM load_events",
  );
  const empty = await owner.run([
    "search",
    "zzzxqvnonexistenttoken20260728",
    "--load",
  ]);
  assert.equal(empty.code, 0, empty.stderr);
  assert.match(empty.stdout, /No matching skills found\./);
  const after = await created.sql.get(
    "SELECT COUNT(*)::int AS n FROM load_events",
  );
  assert.equal(after.n, before.n);

  const invalidConfig = await mkdtemp(
    join(tmpdir(), "flockfly-rust-e2e-invalid-token-"),
  );
  temporaryDirectories.push(invalidConfig);
  await writeFile(
    join(invalidConfig, "credentials.json"),
    `${JSON.stringify({ apiUrl: baseUrl, token: "ffly_invalid" }, null, 2)}\n`,
  );
  const failed = await runProcess(
    ["search", "search load integration sentinel quartz", "--load"],
    {
      FLOCKFLY_API_URL: baseUrl,
      FLOCKFLY_CONFIG_DIR: invalidConfig,
    },
  );
  assert.equal(failed.code, 1);
  assert.match(failed.stderr, /flockfly login/);
});
