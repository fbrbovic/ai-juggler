/**
 * AJ-E802 — visual states harness.
 *
 * Captures the seed-set screens (WorkflowRail with all 9 session
 * statuses, session/inspector panel, onboarding detect) across the
 * required viewport × theme matrix against the mocked bridge with
 * deterministic fixtures. States that do not exist yet are recorded as
 * NOT CAPTURED with the owning task — never silently omitted.
 *
 * Run: pnpm --dir ui visual:capture   (writes docs/qa/evidence/_gallery/)
 */
import { test, expect } from "@playwright/test";
import { mkdirSync, writeFileSync, existsSync, appendFileSync } from "node:fs";
import { resolve } from "node:path";

// Playwright runs with cwd = config dir (ui/).
const GALLERY_ROOT = resolve(process.cwd(), "../docs/qa/evidence/_gallery");
import { allStatuses, honestEmpty, readyHome } from "./fixtures";

const GALLERY = GALLERY_ROOT;
const NOT_CAPTURETED = resolve(GALLERY, "not-captured.md");

const VIEWPORTS = [
  { name: "1440x900", width: 1440, height: 900 },
  { name: "1280x800", width: 1280, height: 800 },
  { name: "1024x680", width: 1024, height: 680 }, // Tauri minimum
  { name: "390x844", width: 390, height: 844 }, // collapsed-rail check
] as const;

const THEMES = ["dark", "light"] as const;

const T0 = 1_728_800_000_000;

function noteNotCaptured(screen: string, state: string, reason: string) {
  appendFileSync(
    NOT_CAPTURETED,
    `| ${screen} | ${state} | NOT CAPTURED: ${reason} |\n`,
  );
}

test.beforeAll(() => {
  mkdirSync(GALLERY, { recursive: true });
  for (const screen of ["01-rail", "01-rail-density", "01-rail-sort", "05-session-panel", "06-inspector", "07-inspector-content", "08-inspector-aggregate", "09-quality", "10-processes", "11-renewal", "12-model", "13-desktop", "14-focus", "15-preview", "16-onboarding-detect", "17-ledger", "18-ports", "19-cleanup", "20-kill", "21-tiers", "22-recovery", "23-alerts", "24-fleet", "25-costs"]) {
    mkdirSync(resolve(GALLERY, screen), { recursive: true });
  }
  writeFileSync(NOT_CAPTURETED, "# Not captured (explicit omissions)\n\n| Screen | State | Reason |\n|---|---|---|\n");
  // Loading/error states do not exist yet — recorded, not hidden.
  for (const screen of ["01-rail", "05-session-panel", "16-onboarding-detect"]) {
    noteNotCaptured(screen, "loading (skeleton)", "pending AJ-E405+ (state emission not yet implemented)");
    noteNotCaptured(screen, "error (banner + retained data)", "pending AJ-E405+ (error states not yet implemented)");
  }
});

for (const theme of THEMES) {
  for (const vp of VIEWPORTS) {
    test(`rail — all statuses — ${vp.name} — ${theme}`, async ({ page }) => {
      await page.addInitScript(
        ({ fixture, t0, theme }) => {
          (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
          localStorage.setItem("aij.theme", theme);
          localStorage.setItem("aij.onboarding.dismissed", "1");
          localStorage.setItem("aij.ready.skipped", "1");
          sessionStorage.setItem("aij.ready.skipped", "1");
          // Pin the clock for deterministic "ago" labels.
          Date.now = () => t0;
        },
        { fixture: allStatuses, t0: T0, theme },
      );
      await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
      await page.setViewportSize({ width: vp.width, height: vp.height });
      await page.goto("/");
      await expect(page.getByRole("tab", { name: "Sessions" })).toBeVisible();
      await page.getByRole("tab", { name: "Sessions" }).click();
      await expect(page.getByText("Handed over").first()).toBeVisible();
      await page.screenshot({
        path: resolve(GALLERY, `01-rail/all-statuses-${vp.name}-${theme}.png`),
        fullPage: false,
      });
    });

    test(`session panel — populated — ${vp.name} — ${theme}`, async ({ page }) => {
      await page.addInitScript(
        ({ fixture, t0, theme }) => {
          (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
          localStorage.setItem("aij.theme", theme);
          localStorage.setItem("aij.onboarding.dismissed", "1");
          sessionStorage.setItem("aij.ready.skipped", "1");
          Date.now = () => t0;
        },
        { fixture: allStatuses, t0: T0, theme },
      );
      await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
      await page.setViewportSize({ width: vp.width, height: vp.height });
      await page.goto("/");
      await expect(page.getByRole("tab", { name: "Sessions" })).toBeVisible();
      await page.getByRole("tab", { name: "Sessions" }).click();
      // The row carries its account in the aria-label (initials disc on
      // screen) — click by accessible name (AJ-E402).
      await page.getByRole("button", { name: /Work Max/ }).first().click();
      await expect(page.getByTestId("handover-section")).toBeVisible();
      await page.screenshot({
        path: resolve(GALLERY, `05-session-panel/populated-${vp.name}-${theme}.png`),
      });
    });

    test(`rail — honest empty — ${vp.name} — ${theme}`, async ({ page }) => {
      await page.addInitScript(
        ({ fixture, theme }) => {
          (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
          localStorage.setItem("aij.theme", theme);
          localStorage.setItem("aij.onboarding.dismissed", "1");
          sessionStorage.setItem("aij.ready.skipped", "1");
        },
        { fixture: honestEmpty, theme },
      );
      await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
      await page.setViewportSize({ width: vp.width, height: vp.height });
      await page.goto("/");
      // AJ-E406: the empty work area is the fleet-at-a-glance summary
      // (RightInspector, Req.4) — never a blank panel.
      await expect(page.getByText("Nothing selected", { exact: false })).toBeVisible();
      await expect(page.getByText("Lowest quota")).toBeVisible();
      await page.screenshot({
        path: resolve(GALLERY, `01-rail/honest-empty-${vp.name}-${theme}.png`),
      });
    });

    test(`ready home — ${vp.name} — ${theme}`, async ({ page }) => {
      await page.addInitScript(
        ({ fixture, theme }) => {
          (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
          localStorage.setItem("aij.theme", theme);
          localStorage.removeItem("aij.onboarding.dismissed");
        },
        { fixture: readyHome, theme },
      );
      await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
      await page.setViewportSize({ width: vp.width, height: vp.height });
      await page.goto("/");
      await expect(page.getByText("Get your accounts ready")).toBeVisible();
      await page.screenshot({
        path: resolve(GALLERY, `01-rail/ready-home-${vp.name}-${theme}.png`),
      });
    });

    test(`onboarding detect — ${vp.name} — ${theme}`, async ({ page }) => {
      await page.addInitScript(
        ({ fixture, theme }) => {
          (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
          localStorage.setItem("aij.theme", theme);
          localStorage.setItem("aij.onboarding.dismissed", "1");
          sessionStorage.setItem("aij.ready.skipped", "1");
        },
        { fixture: honestEmpty, theme },
      );
      await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
      await page.setViewportSize({ width: vp.width, height: vp.height });
      await page.goto("/");
      await page.getByRole("button", { name: "Accounts" }).click();
      await page.getByRole("button", { name: /Add account/ }).first().click();
      await expect(page.getByText("let's look at what you already have", { exact: false })).toBeVisible();
      await page.waitForTimeout(400); // scan mock resolves
      await expect(page.getByText(/signed in|not signed in/).first()).toBeVisible();
      await page.screenshot({
        path: resolve(GALLERY, `16-onboarding-detect/populated-${vp.name}-${theme}.png`),
      });
    });
  }
}

// AJ-E403 density captures: 3 densities × (1440x900, 1024x768), the
// Minimal hover tooltip, and one stale row (freshness forced old via the
// __AIJ_SAMPLED_AT__ fixture hook).
const DENSITY_VIEWPORTS = [
  { name: "1440x900", width: 1440, height: 900 },
  { name: "1024x768", width: 1024, height: 768 },
] as const;
const DENSITIES = ["comfortable", "compact", "minimal"] as const;

for (const density of DENSITIES) {
  for (const vp of DENSITY_VIEWPORTS) {
    test(`rail density ${density} — ${vp.name}`, async ({ page }) => {
      await page.addInitScript(
        ({ fixture, t0, mode }) => {
          (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
          localStorage.setItem("aij.theme", "dark");
          localStorage.setItem("aij.density", mode);
          localStorage.setItem("aij.onboarding.dismissed", "1");
          sessionStorage.setItem("aij.ready.skipped", "1");
          Date.now = () => t0;
        },
        { fixture: allStatuses, t0: T0, mode: density },
      );
      await page.emulateMedia({ reducedMotion: "reduce" });
      await page.setViewportSize({ width: vp.width, height: vp.height });
      await page.goto("/");
      await expect(page.getByRole("tab", { name: "Sessions" })).toBeVisible();
      await page.getByRole("tab", { name: "Sessions" }).click();
      if (density === "minimal") {
        await expect(page.getByTitle(/Work Max/).first()).toBeVisible();
      } else {
        await expect(page.getByText("Handed over").first()).toBeVisible();
      }
      await page.screenshot({
        path: resolve(GALLERY, `01-rail-density/${density}-${vp.name}.png`),
      });
    });
  }
}

test("rail minimal — hover tooltip open", async ({ page }) => {
  await page.addInitScript(
    ({ fixture, t0 }) => {
      (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
      localStorage.setItem("aij.theme", "dark");
      localStorage.setItem("aij.density", "minimal");
      localStorage.setItem("aij.onboarding.dismissed", "1");
      sessionStorage.setItem("aij.ready.skipped", "1");
      Date.now = () => t0;
    },
    { fixture: allStatuses, t0: T0 },
  );
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/");
  await expect(page.getByRole("tab", { name: "Sessions" })).toBeVisible();
  await page.getByRole("tab", { name: "Sessions" }).click();
  const first = page.locator("aside button[title]").first();
  await first.hover();
  await page.waitForTimeout(150);
  await page.screenshot({ path: resolve(GALLERY, "01-rail-density/minimal-tooltip-1440x900.png") });
});

test("rail — stale row renders stale - Nm", async ({ page }) => {
  await page.addInitScript(
    ({ fixture, t0 }) => {
      (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
      // The running instance's samples are 3 minutes old — past 2× the
      // 5 s resource and 30 s quota intervals → `stale - 3m`.
      (window as never as { __AIJ_SAMPLED_AT__: unknown }).__AIJ_SAMPLED_AT__ = {
        "sess-2": t0 - 3 * 60_000,
      };
      localStorage.setItem("aij.theme", "dark");
      localStorage.setItem("aij.onboarding.dismissed", "1");
      sessionStorage.setItem("aij.ready.skipped", "1");
      Date.now = () => t0;
    },
    { fixture: allStatuses, t0: T0 },
  );
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/");
  await expect(page.getByRole("tab", { name: "Sessions" })).toBeVisible();
  await page.getByRole("tab", { name: "Sessions" }).click();
  await expect(page.getByText("stale - 3m").first()).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "01-rail-density/stale-row-1440x900.png") });
});

// AJ-E404 captures: the sorted/filtered rail states (1440x900).
test("rail — needs-me sort default", async ({ page }) => {
  await seedRail(page, {});
  await expect(page.getByText("Handed over").first()).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "01-rail-sort/needs-me-1440x900.png") });
});

test("rail — quota risk with no data group", async ({ page }) => {
  await seedRail(page, {});
  await page.selectOption('select[aria-label="Sort by"]', "quota");
  await expect(page.getByText("no data").first()).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "01-rail-sort/quota-risk-1440x900.png") });
});

test("rail — group by project with rollups", async ({ page }) => {
  await seedRail(page, {});
  await page.selectOption('select[aria-label="Group by"]', "project");
  await expect(page.getByText(/running/).first()).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "01-rail-sort/group-by-project-1440x900.png") });
});

test("rail — empty search state", async ({ page }) => {
  await seedRail(page, {});
  await page.locator('input[type="search"]').fill("does-not-exist");
  await expect(page.getByText("No instances match")).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "01-rail-sort/empty-search-1440x900.png") });
});

test("rail — snoozed badge (snoozed 12m)", async ({ page }) => {
  const t0 = T0;
  await seedRail(page, {
    snooze: { "sess-3": { until: t0 + 12 * 60_000, muted: false } },
  });
  await expect(page.getByText("snoozed 12m").first()).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "01-rail-sort/snoozed-1440x900.png") });
});

/** Shared rail seeding for the sort/filter captures. */
async function seedRail(
  page: import("@playwright/test").Page,
  options: { snooze?: Record<string, { until: number | null; muted: boolean }> },
) {
  await page.addInitScript(
    ({ fixture, t0, snooze }) => {
      (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
      if (snooze) localStorage.setItem("aij.snooze", JSON.stringify(snooze));
      localStorage.setItem("aij.theme", "dark");
      localStorage.setItem("aij.onboarding.dismissed", "1");
      sessionStorage.setItem("aij.ready.skipped", "1");
      Date.now = () => t0;
    },
    { fixture: allStatuses, t0: T0, snooze: options.snooze },
  );
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/");
  await expect(page.getByRole("tab", { name: "Sessions" })).toBeVisible();
  await page.getByRole("tab", { name: "Sessions" }).click();
}

// AJ-E406 inspector captures: the RightInspector shell across selection
// types (empty summary, Instance/Overview, Account/Usage honest empty,
// Project/Processes header) at 1440x900 + 1280x800.
const INSPECTOR_VIEWPORTS = [
  { name: "1440x900", width: 1440, height: 900 },
  { name: "1280x800", width: 1280, height: 800 },
] as const;

async function seedInspectorPage(page: import("@playwright/test").Page, vp: { width: number; height: number }) {
  await page.addInitScript(
    ({ fixture, t0 }) => {
      (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
      localStorage.setItem("aij.theme", "dark");
      localStorage.setItem("aij.onboarding.dismissed", "1");
      sessionStorage.setItem("aij.ready.skipped", "1");
      Date.now = () => t0;
    },
    { fixture: allStatuses, t0: T0 },
  );
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.setViewportSize({ width: vp.width, height: vp.height });
  await page.goto("/");
}

for (const vp of INSPECTOR_VIEWPORTS) {
  test(`inspector — fleet-at-a-glance (empty) — ${vp.name}`, async ({ page }) => {
    await seedInspectorPage(page, vp);
    await expect(page.getByText("Nothing selected", { exact: false })).toBeVisible();
    await page.screenshot({ path: resolve(GALLERY, `06-inspector/empty-${vp.name}.png`) });
  });

  test(`inspector — Instance / Overview — ${vp.name}`, async ({ page }) => {
    await seedInspectorPage(page, vp);
    await page.getByRole("tab", { name: "Sessions" }).click();
    await page.getByRole("button", { name: /Work Max/ }).first().click();
    await expect(page.getByRole("tab", { name: "Quality / Performance" })).toBeVisible();
    await page.screenshot({ path: resolve(GALLERY, `06-inspector/instance-overview-${vp.name}.png`) });
  });

  test(`inspector — Account / Usage honest empty — ${vp.name}`, async ({ page }) => {
    await seedInspectorPage(page, vp);
    await page.getByRole("button", { name: "Accounts" }).click();
    await page.getByRole("button", { name: "Inspect claude-code" }).first().click();
    await expect(page.getByRole("tab", { name: "Usage" })).toBeVisible();
    await page.getByRole("tab", { name: "Usage" }).click();
    await expect(page.getByText("No measured usage yet", { exact: false })).toBeVisible();
    await page.screenshot({ path: resolve(GALLERY, `06-inspector/account-usage-${vp.name}.png`) });
  });

  test(`inspector — Project / Processes — ${vp.name}`, async ({ page }) => {
    await seedInspectorPage(page, vp);
    await page.getByRole("tab", { name: "Projects" }).click();
    await page.getByRole("button", { name: /ai-juggler/ }).first().click();
    await expect(page.getByRole("tab", { name: "Processes" })).toBeVisible();
    await page.screenshot({ path: resolve(GALLERY, `06-inspector/project-processes-${vp.name}.png`) });
  });
}

// AJ-E407 inspector-content captures (Instance + Session tabs).
test("inspector — Instance Overview fields (populated)", async ({ page }) => {
  await seedInspectorPage(page, { width: 1440, height: 900 });
  await page.getByRole("tab", { name: "Sessions" }).click();
  await page.getByRole("button", { name: /Work Max/ }).first().click();
  await expect(page.getByTestId("instance-overview-fields")).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "07-inspector-content/instance-overview-populated.png") });
});

test("inspector — Instance Usage honest chart empties", async ({ page }) => {
  await seedInspectorPage(page, { width: 1440, height: 900 });
  await page.getByRole("tab", { name: "Sessions" }).click();
  await page.getByRole("button", { name: /Work Max/ }).first().click();
  await page.getByRole("tab", { name: "Usage" }).click();
  await expect(page.locator('section:not([hidden])').getByText('No data — arrives with statusline').first()).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "07-inspector-content/instance-usage.png") });
});

test("inspector — Instance Actions disabled-with-reason", async ({ page }) => {
  await seedInspectorPage(page, { width: 1440, height: 900 });
  await page.getByRole("tab", { name: "Sessions" }).click();
  await page.getByRole("button", { name: /Work Max/ }).first().click();
  await page.getByRole("tab", { name: "Actions" }).click();
  await expect(page.getByText("AJ-E202").first()).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "07-inspector-content/instance-actions.png") });
});

test("inspector — Session Overview ended stamp", async ({ page }) => {
  await seedInspectorPage(page, { width: 1440, height: 900 });
  await page.getByRole("tab", { name: "Sessions" }).click();
  await page.getByRole("button", { name: /Personal/ }).first().click();
  await expect(page.getByText(/Ended|Conversation|Transcript not readable yet/).first()).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "07-inspector-content/session-overview.png") });
});

// AJ-E408 aggregate-content captures.
test("inspector — Account Overview rings + forecast", async ({ page }) => {
  await seedInspectorPage(page, { width: 1440, height: 900 });
  await page.getByRole("button", { name: "Accounts" }).click();
  await page.getByRole("button", { name: "Inspect claude-code" }).first().click();
  await expect(page.getByTestId("account-rings")).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "08-inspector-aggregate/account-overview-1440x900.png") });
});

test("inspector — Account logged-out collapse", async ({ page }) => {
  // Override: the Personal account is signed out in this capture.
  await page.addInitScript(
    ({ fixture, t0 }) => {
      (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = {
        ...fixture,
        accounts: fixture.accounts.map((a: { id: string }) =>
          a.id === "claude-personal" ? { ...a, login_state: "logged_out" } : a,
        ),
      };
      localStorage.setItem("aij.theme", "dark");
      localStorage.setItem("aij.onboarding.dismissed", "1");
      sessionStorage.setItem("aij.ready.skipped", "1");
      Date.now = () => t0;
    },
    { fixture: allStatuses, t0: T0 },
  );
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.getByRole("button", { name: "Accounts" }).click();
  await page.getByRole("button", { name: "Inspect claude-code" }).nth(1).click();
  await expect(page.getByTestId("account-signin-card")).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "08-inspector-aggregate/account-logged-out-1280x800.png") });
});

test("inspector — Project Overview On-it-now card", async ({ page }) => {
  await seedInspectorPage(page, { width: 1440, height: 900 });
  await page.getByRole("tab", { name: "Projects" }).click();
  await page.getByRole("button", { name: /ai-juggler/ }).first().click();
  await expect(page.getByTestId("project-on-it-now")).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "08-inspector-aggregate/project-overview-1440x900.png") });
});

// AJ-E409 quality-tab captures.
test("inspector — Quality tab populated", async ({ page }) => {
  await seedInspectorPage(page, { width: 1440, height: 900 });
  await page.getByRole("tab", { name: "Sessions" }).click();
  await page.getByRole("button", { name: /Work Max/ }).first().click();
  await page.getByRole("tab", { name: "Quality / Performance" }).click();
  await expect(page.getByTestId("quality-tokens")).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "09-quality/populated-1440x900.png") });
});

test("inspector — Quality tab View as table", async ({ page }) => {
  await seedInspectorPage(page, { width: 1440, height: 900 });
  await page.getByRole("tab", { name: "Sessions" }).click();
  await page.getByRole("button", { name: /Work Max/ }).first().click();
  await page.getByRole("tab", { name: "Quality / Performance" }).click();
  await page.getByRole("button", { name: "View as table" }).click();
  await expect(page.getByTestId("quality-tokens").getByRole("table")).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "09-quality/as-table-1440x900.png") });
});

// AJ-E410 processes-tab captures: live tree (3 levels), orphan section
// with the PID-named confirm, the `gone` verification state, and the
// good empty state.
test("inspector — Processes tab live tree + orphans", async ({ page }) => {
  await seedInspectorPage(page, { width: 1440, height: 900 });
  await page.getByRole("tab", { name: "Sessions" }).click();
  await page.getByRole("button", { name: /Work Max/ }).first().click();
  await page.getByRole("tab", { name: "Processes" }).click();
  await expect(page.getByTestId("processes-spark")).toBeVisible();
  await expect(page.getByTestId("orphan-row-39871")).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "10-processes/live-tree-1440x900.png") });
});

test("inspector — Processes tab Stop confirm popover (PID named)", async ({ page }) => {
  await seedInspectorPage(page, { width: 1440, height: 900 });
  await page.getByRole("tab", { name: "Sessions" }).click();
  await page.getByRole("button", { name: /Work Max/ }).first().click();
  await page.getByRole("tab", { name: "Processes" }).click();
  await expect(page.getByTestId("orphan-row-39871")).toBeVisible();
  await page.getByRole("button", { name: "Stop process 39871" }).click();
  await expect(page.getByTestId("orphan-confirm-39871")).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "10-processes/stop-confirm-1440x900.png") });
});

test("inspector — Processes tab gone verification state", async ({ page }) => {
  await seedInspectorPage(page, { width: 1440, height: 900 });
  await page.getByRole("tab", { name: "Sessions" }).click();
  await page.getByRole("button", { name: /Work Max/ }).first().click();
  await page.getByRole("tab", { name: "Processes" }).click();
  await expect(page.getByTestId("orphan-row-39871")).toBeVisible();
  await page.getByRole("button", { name: "Stop process 39871" }).click();
  await page.getByRole("button", { name: "Stop 39871" }).click();
  await expect(page.getByTestId("orphan-state-39871")).toHaveText("gone ✓");
  await page.screenshot({ path: resolve(GALLERY, "10-processes/gone-state-1440x900.png") });
});

test("inspector — Processes tab good empty state", async ({ page }) => {
  await seedInspectorPage(page, { width: 1440, height: 900 });
  await page.getByRole("tab", { name: "Sessions" }).click();
  await page.getByRole("button", { name: /Personal/ }).first().click();
  await page.getByRole("tab", { name: "Processes" }).click();
  await expect(page.getByTestId("processes-empty")).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "10-processes/empty-1440x900.png") });
});

// AJ-E411 renewal captures: every threshold state on the Account
// Overview, `Expiry: unknown`, the editor + helper text, and the row
// chip on the Accounts screen. Dates are pinned relative to T0 so the
// tiers are deterministic.
function renewalDate(daysFromT0: number): string {
  const target = new Date(T0 + daysFromT0 * 86_400_000);
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${target.getFullYear()}-${pad(target.getMonth() + 1)}-${pad(target.getDate())}`;
}

async function seedRenewalPage(
  page: import("@playwright/test").Page,
  vp: { width: number; height: number },
  days: number | null,
) {
  await page.addInitScript(
    ({ fixture, t0, days: d }) => {
      const accounts = (fixture.accounts ?? []).map((entry: { id: string }, index: number) => ({
        ...entry,
        renewal_date: index === 0 ? (d === null ? null : new Date(t0 + d * 86_400_000).toISOString().slice(0, 10)) : null,
      }));
      (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = { ...fixture, accounts };
      localStorage.setItem("aij.theme", "dark");
      localStorage.setItem("aij.onboarding.dismissed", "1");
      sessionStorage.setItem("aij.ready.skipped", "1");
      Date.now = () => t0;
    },
    { fixture: allStatuses, t0: T0, days },
  );
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.setViewportSize({ width: vp.width, height: vp.height });
  await page.goto("/");
}

async function openAccountOverview(page: import("@playwright/test").Page) {
  await page.getByRole("button", { name: "Accounts" }).click();
  await page.getByRole("button", { name: "Inspect claude-code" }).first().click();
  await expect(page.getByTestId("account-overview")).toBeVisible();
}

for (const [name, days] of [
  ["date-only", 20],
  ["countdown", 10],
  ["warning", 6],
  ["danger", 0],
  ["unknown", null],
] as const) {
  test(`renewal — Account Overview ${name} state`, async ({ page }) => {
    await seedRenewalPage(page, { width: 1440, height: 900 }, days);
    await openAccountOverview(page);
    await expect(page.getByTestId("account-renewal")).toBeVisible();
    await page.screenshot({ path: resolve(GALLERY, `11-renewal/account-${name}-1440x900.png`) });
  });
}

test("renewal — editor with helper text (1440x900 + 1280x800)", async ({ page }) => {
  await seedRenewalPage(page, { width: 1440, height: 900 }, 6);
  await openAccountOverview(page);
  await expect(page.getByTestId("renewal-editor")).toBeVisible();
  await expect(page.getByText("Entered by you - Juggler does not read billing")).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "11-renewal/editor-1440x900.png") });
});

test("renewal — editor at 1280x800", async ({ page }) => {
  await seedRenewalPage(page, { width: 1280, height: 800 }, 6);
  await openAccountOverview(page);
  await expect(page.getByTestId("renewal-editor")).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "11-renewal/editor-1280x800.png") });
});

test("renewal — Accounts row chip at <= 7 days (and none for unknown)", async ({ page }) => {
  await seedRenewalPage(page, { width: 1440, height: 900 }, 6);
  await page.getByRole("button", { name: "Accounts" }).click();
  await expect(page.getByText("Renews in 6 days").first()).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "11-renewal/accounts-row-chip-1440x900.png") });
});

// AJ-E412 running-model captures: I-03 plain vs drift (sess-8 carries
// the drift fixture: launched opus, observed sonnet-5), the Overview
// dual-model line, and the not-reported inspector state.
test("model — I-03 drift marker on the rail row", async ({ page }) => {
  await seedInspectorPage(page, { width: 1440, height: 900 });
  await page.getByRole("tab", { name: "Sessions" }).click();
  const marker = page.getByTestId("model-drift").first();
  await expect(marker).toBeVisible();
  await expect(marker).toHaveAttribute("aria-label", "Launched with opus - now running sonnet-5");
  await marker.hover();
  await page.screenshot({ path: resolve(GALLERY, "12-model/rail-drift-1440x900.png") });
});

test("model — I-03 plain (launch == running rows show no marker)", async ({ page }) => {
  await seedInspectorPage(page, { width: 1440, height: 900 });
  await page.getByRole("tab", { name: "Sessions" }).click();
  // The plain rows carry launch-only models; the marker count is exactly
  // the drift rows (one), asserted before the screenshot.
  expect(await page.getByTestId("model-drift").count()).toBe(1);
  await page.screenshot({ path: resolve(GALLERY, "12-model/rail-plain-1440x900.png") });
});

test("model — Instance Overview dual-model line", async ({ page }) => {
  await seedInspectorPage(page, { width: 1440, height: 900 });
  await page.getByRole("tab", { name: "Sessions" }).click();
  await page.getByRole("button", { name: /Work Max/ }).first().click();
  await expect(page.getByTestId("instance-model-line")).toBeVisible();
  await expect(page.getByTestId("instance-model-line")).toContainText(
    "Launched with opus - now running sonnet-5",
  );
  await page.screenshot({ path: resolve(GALLERY, "12-model/overview-drift-1440x900.png") });
});

test("model — not reported by Provider inspector state", async ({ page }) => {
  await seedInspectorPage(page, { width: 1440, height: 900 });
  await page.getByRole("tab", { name: "Sessions" }).click();
  await page.getByRole("button", { name: /Personal/ }).first().click();
  await expect(page.getByTestId("instance-model-line")).toContainText(
    "Model: not reported by claude-code",
  );
  await page.screenshot({ path: resolve(GALLERY, "12-model/not-reported-1440x900.png") });
});

// AJ-E413 desktop captures: the panel surface (no terminal card), the
// rail `app` chip, and the state matrix (running, exited, crashed,
// single-account warning) at the two required viewports.
test("desktop — Instance panel (running app, isolation label, no terminal)", async ({ page }) => {
  await seedInspectorPage(page, { width: 1280, height: 800 });
  await page.getByRole("tab", { name: "Sessions" }).click();
  await page
    .getByRole("button", { name: /Fake Desk/ })
    .filter({ has: page.getByTestId("rail-app-chip") })
    .filter({ hasText: /running/i })
    .click();
  await expect(page.getByTestId("desktop-surface")).toBeVisible();
  await expect(page.getByTestId("desktop-surface")).toContainText("Running");
  await expect(page.getByTestId("desktop-focus")).toBeVisible();
  await expect(page.getByTestId("desktop-isolation")).toContainText("Single-account app");
  await page.screenshot({ path: resolve(GALLERY, "13-desktop/panel-running-1280x800.png") });
});

test("desktop — panel at 1024x680 light", async ({ page }) => {
  await page.addInitScript(
    ({ fixture, t0 }) => {
      (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
      localStorage.setItem("aij.theme", "light");
      localStorage.setItem("aij.onboarding.dismissed", "1");
      sessionStorage.setItem("aij.ready.skipped", "1");
      Date.now = () => t0;
    },
    { fixture: allStatuses, t0: T0 },
  );
  await page.emulateMedia({ reducedMotion: "reduce", colorScheme: "light" });
  await page.setViewportSize({ width: 1024, height: 680 });
  await page.goto("/");
  await page.getByRole("tab", { name: "Sessions" }).click();
  await page
    .getByRole("button", { name: /Fake Desk/ })
    .filter({ has: page.getByTestId("rail-app-chip") })
    .filter({ hasText: /running/i })
    .click();
  await expect(page.getByTestId("desktop-surface")).toBeVisible();
  await expect(page.getByTestId("desktop-isolation")).toContainText("Single-account app");
  await page.screenshot({ path: resolve(GALLERY, "13-desktop/panel-running-1024x680-light.png") });
});

test("desktop — exited app state", async ({ page }) => {
  await seedInspectorPage(page, { width: 1280, height: 800 });
  await page.getByRole("tab", { name: "Sessions" }).click();
  await page
    .getByRole("button", { name: /Fake Desk/ })
    .filter({ has: page.getByTestId("rail-app-chip") })
    .filter({ hasText: /finished/i })
    .click();
  await expect(page.getByTestId("desktop-surface")).toBeVisible();
  await expect(page.getByTestId("desktop-surface")).toContainText("app exited");
  await page.screenshot({ path: resolve(GALLERY, "13-desktop/panel-exited-1280x800.png") });
});

test("desktop — crashed app state", async ({ page }) => {
  await seedInspectorPage(page, { width: 1280, height: 800 });
  await page.getByRole("tab", { name: "Sessions" }).click();
  await page
    .getByRole("button", { name: /Fake Desk/ })
    .filter({ has: page.getByTestId("rail-app-chip") })
    .filter({ hasText: /failed|crashed/i })
    .click();
  await expect(page.getByTestId("desktop-surface")).toBeVisible();
  await expect(page.getByTestId("desktop-surface")).toContainText("app crashed");
  await page.screenshot({ path: resolve(GALLERY, "13-desktop/panel-crashed-1280x800.png") });
});

test("desktop — single-account warning state", async ({ page }) => {
  await seedInspectorPage(page, { width: 1280, height: 800 });
  await page.getByRole("tab", { name: "Sessions" }).click();
  await page
    .getByRole("button", { name: /Fake Desk/ })
    .filter({ has: page.getByTestId("rail-app-chip") })
    .filter({ hasText: /running/i })
    .click();
  const warning = page.getByTestId("desktop-isolation");
  await expect(warning).toBeVisible();
  await expect(warning).toContainText(
    "Single-account app — Juggler tracks, cannot isolate",
  );
  await page.screenshot({ path: resolve(GALLERY, "13-desktop/panel-single-account-warning-1280x800.png") });
});

test("desktop — rail app chip", async ({ page }) => {
  await seedInspectorPage(page, { width: 1280, height: 800 });
  await page.getByRole("tab", { name: "Sessions" }).click();
  await expect(page.getByTestId("rail-app-chip").first()).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "13-desktop/rail-app-chip-1280x800.png") });
});

// AJ-E414 focus captures: the outcome text after a focus attempt, the
// permission explainer (degraded tier), and the "Go to instance…"
// palette at both themes. The harness seam `__AIJ_FOCUS_OUTCOME__`
// pins the mocked tier result each capture renders.
async function seedFocusPage(
  page: import("@playwright/test").Page,
  outcome: { outcome: string; permission_missing: boolean; window_title: string | null },
) {
  await page.addInitScript(
    ({ fixture, t0, pinned }) => {
      (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
      (window as never as { __AIJ_FOCUS_OUTCOME__: unknown }).__AIJ_FOCUS_OUTCOME__ = pinned;
      localStorage.setItem("aij.theme", "dark");
      localStorage.setItem("aij.onboarding.dismissed", "1");
      sessionStorage.setItem("aij.ready.skipped", "1");
      Date.now = () => t0;
    },
    {
      fixture: allStatuses,
      t0: T0,
      pinned: outcome,
    },
  );
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
}

test("focus — panel outcome after focus (Focused window, dark)", async ({ page }) => {
  await seedFocusPage(page, {
    outcome: "Focused window",
    permission_missing: false,
    window_title: "aij-e414-a.txt",
  });
  await page.getByRole("tab", { name: "Sessions" }).click();
  await page
    .getByRole("button", { name: /Fake Desk/ })
    .filter({ has: page.getByTestId("rail-app-chip") })
    .filter({ hasText: /running/i })
    .click();
  await page.getByTestId("desktop-focus").click();
  await expect(page.getByTestId("desktop-focus-outcome")).toContainText("Focused window");
  await expect(page.getByTestId("desktop-focus-outcome")).toContainText("aij-e414-a.txt");
  await expect(page.getByTestId("desktop-a11y-explainer")).toHaveCount(0);
  await page.screenshot({ path: resolve(GALLERY, "14-focus/panel-outcome-1280x800.png") });
});

test("focus — permission explainer on the degraded tier", async ({ page }) => {
  await seedFocusPage(page, {
    outcome: "App opened — window not found",
    permission_missing: true,
    window_title: null,
  });
  await page.getByRole("tab", { name: "Sessions" }).click();
  await page
    .getByRole("button", { name: /Fake Desk/ })
    .filter({ has: page.getByTestId("rail-app-chip") })
    .filter({ hasText: /running/i })
    .click();
  await page.getByTestId("desktop-focus").click();
  const explainer = page.getByTestId("desktop-a11y-explainer");
  await expect(explainer).toBeVisible();
  await expect(explainer).toContainText(
    "Juggler can open the app; exact-window focus needs Accessibility permission",
  );
  await page.screenshot({ path: resolve(GALLERY, "14-focus/permission-explainer-1280x800.png") });
});

test("focus — Go to instance… palette (dark 1280x800)", async ({ page }) => {
  await seedInspectorPage(page, { width: 1280, height: 800 });
  await page.keyboard.press("Meta+k");
  const palette = page.getByTestId("goto-palette");
  await expect(palette).toBeVisible();
  // Live instances only, desktop first; the footer names the binding.
  await expect(palette.locator('[data-testid="goto-row-sess-desktop-1"]')).toBeVisible();
  await expect(palette.locator('[data-testid="goto-row-sess-2"]')).toBeVisible();
  await expect(palette.locator('[data-testid="goto-row-sess-desktop-3"]')).toHaveCount(0);
  await expect(palette.locator('[data-testid="goto-row-sess-6"]')).toHaveCount(0);
  await expect(palette).toContainText("Alt+Space");
  await page.screenshot({ path: resolve(GALLERY, "14-focus/palette-1280x800.png") });
});

test("focus — Go to instance… palette (light 1024x680)", async ({ page }) => {
  await page.addInitScript(
    ({ fixture, t0 }) => {
      (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
      localStorage.setItem("aij.theme", "light");
      localStorage.setItem("aij.onboarding.dismissed", "1");
      sessionStorage.setItem("aij.ready.skipped", "1");
      Date.now = () => t0;
    },
    { fixture: allStatuses, t0: T0 },
  );
  await page.emulateMedia({ reducedMotion: "reduce", colorScheme: "light" });
  await page.setViewportSize({ width: 1024, height: 680 });
  await page.goto("/");
  await page.keyboard.press("Meta+k");
  const palette = page.getByTestId("goto-palette");
  await expect(palette).toBeVisible();
  await expect(palette.locator('[data-testid="goto-row-sess-desktop-1"]')).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "14-focus/palette-1024x680-light.png") });
});

// AJ-E415 preview captures: the live mirror (indicator + frame age +
// click-to-focus affordance), the paused (minimised) state, and the
// permission-denied state — both themes. The harness seam
// `__AIJ_PREVIEW_PHASE__` pins the phase each capture renders.
async function seedPreviewPage(
  page: import("@playwright/test").Page,
  vp: { width: number; height: number },
  theme: "dark" | "light",
  phase: { phase: string; text: string } | null,
) {
  await page.addInitScript(
    ({ fixture, t0, themeName, seededPhase }) => {
      (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
      if (seededPhase !== null) {
        (window as never as { __AIJ_PREVIEW_PHASE__: unknown }).__AIJ_PREVIEW_PHASE__ = {
          session_id: "sess-desktop-1",
          ...seededPhase,
        };
      }
      localStorage.setItem("aij.theme", themeName);
      localStorage.setItem("aij.onboarding.dismissed", "1");
      sessionStorage.setItem("aij.ready.skipped", "1");
      Date.now = () => t0;
    },
    { fixture: allStatuses, t0: T0, themeName: theme, seededPhase: phase },
  );
  await page.emulateMedia({
    reducedMotion: "reduce",
    colorScheme: theme === "light" ? "light" : "dark",
  });
  await page.setViewportSize({ width: vp.width, height: vp.height });
  await page.goto("/");
}

async function openRunningDesktopPanel(page: import("@playwright/test").Page) {
  await page.getByRole("tab", { name: "Sessions" }).click();
  await page
    .getByRole("button", { name: /Fake Desk/ })
    .filter({ has: page.getByTestId("rail-app-chip") })
    .filter({ hasText: /running/i })
    .click();
}

test("preview — live mirror with indicator, frame age, click-to-focus (dark)", async ({ page }) => {
  await seedPreviewPage(page, { width: 1280, height: 800 }, "dark", null);
  await openRunningDesktopPanel(page);
  const preview = page.getByTestId("desktop-preview");
  await expect(preview).toBeVisible();
  await expect(preview.locator('[data-testid="preview-live-indicator"]')).toContainText("Live preview");
  await expect(preview.locator('[data-testid="preview-frame-age"]')).toContainText("frame age");
  const frame = preview.locator('[data-testid="preview-frame"]');
  await expect(frame).toBeVisible();
  await expect(preview).toContainText("click to focus the real window");
  await page.screenshot({ path: resolve(GALLERY, "15-preview/panel-live-1280x800.png") });
});

test("preview — live mirror at 1024x680 light", async ({ page }) => {
  await seedPreviewPage(page, { width: 1024, height: 680 }, "light", null);
  await openRunningDesktopPanel(page);
  const preview = page.getByTestId("desktop-preview");
  await expect(preview.locator('[data-testid="preview-frame"]')).toBeVisible();
  await expect(preview.locator('[data-testid="preview-live-indicator"]')).toContainText("Live preview");
  await page.screenshot({ path: resolve(GALLERY, "15-preview/panel-live-1024x680-light.png") });
});

test("preview — paused (window minimised) keeps the labelled last frame", async ({ page }) => {
  await seedPreviewPage(page, { width: 1280, height: 800 }, "dark", {
    phase: "paused",
    text: "Preview paused — window not visible",
  });
  await openRunningDesktopPanel(page);
  const preview = page.getByTestId("desktop-preview");
  await expect(preview.locator('[data-testid="preview-paused"]')).toHaveText(
    "Preview paused — window not visible",
  );
  await expect(preview.locator('[data-testid="preview-frame"]')).toBeVisible();
  await page.screenshot({ path: resolve(GALLERY, "15-preview/panel-paused-1280x800.png") });
});

test("preview — permission denied renders guidance, no frame", async ({ page }) => {
  await seedPreviewPage(page, { width: 1280, height: 800 }, "dark", {
    phase: "denied",
    text: "Screen Recording permission needed — grant it in System Settings, then re-enable preview",
  });
  await openRunningDesktopPanel(page);
  const preview = page.getByTestId("desktop-preview");
  await expect(preview.locator('[data-testid="preview-denied"]')).toContainText(
    "Screen Recording permission needed",
  );
  await expect(preview.locator('[data-testid="preview-frame"]')).toHaveCount(0);
  await page.screenshot({ path: resolve(GALLERY, "15-preview/panel-denied-1280x800.png") });
});

test("preview — exited instance renders no preview (status only)", async ({ page }) => {
  await seedPreviewPage(page, { width: 1280, height: 800 }, "dark", null);
  await page.getByRole("tab", { name: "Sessions" }).click();
  await page
    .getByRole("button", { name: /Fake Desk/ })
    .filter({ has: page.getByTestId("rail-app-chip") })
    .filter({ hasText: /finished/i })
    .click();
  await expect(page.getByTestId("desktop-surface")).toContainText("app exited");
  await expect(page.getByTestId("desktop-preview")).toHaveCount(0);
  await page.screenshot({ path: resolve(GALLERY, "15-preview/panel-exited-1280x800.png") });
});

// AJ-E501: the ledger-degraded banner (the Cleanup Centre stub's only
// surface) — copy exact, both themes, on the Overview screen.
for (const theme of ["dark", "light"] as const) {
  test(`ledger — degraded banner (${theme} 1440x900)`, async ({ page }) => {
    await page.addInitScript(
      ({ fixture, t0, themeName }) => {
        const seeded = { ...fixture };
        seeded.stats = {
          accounts: 3,
          profiles: 3,
          active_sessions: 3,
          ledger_degraded: true,
        };
        (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = seeded;
        localStorage.setItem("aij.theme", themeName);
        localStorage.setItem("aij.onboarding.dismissed", "1");
        sessionStorage.setItem("aij.ready.skipped", "1");
        Date.now = () => t0;
      },
      { fixture: allStatuses, t0: T0, themeName: theme },
    );
    await page.emulateMedia({
      reducedMotion: "reduce",
      colorScheme: theme === "light" ? "light" : "dark",
    });
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto("/");
    await page.getByRole("button", { name: "Overview" }).click();
    const banner = page.getByTestId("ledger-degraded-banner");
    await expect(banner).toBeVisible();
    await expect(banner).toContainText(
      "Process tracking degraded — cleanup is read-only until restart.",
    );
    await page.screenshot({
      path: resolve(GALLERY, `17-ledger/degraded-banner-1440x900-${theme}.png`),
    });
  });
}

// Determinism self-test (Req.4): same fixture twice → byte-identical PNG.
test("determinism — two captures are byte-identical", async ({ page }) => {  const capture = async (suffix: string) => {
    await page.addInitScript(
      ({ fixture, t0 }) => {
        (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
        localStorage.setItem("aij.theme", "dark");
        localStorage.setItem("aij.onboarding.dismissed", "1");
        sessionStorage.setItem("aij.ready.skipped", "1");
        Date.now = () => t0;
      },
      { fixture: allStatuses, t0: T0 },
    );
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.goto("/");
    await expect(page.getByRole("tab", { name: "Sessions" })).toBeVisible();
    const path = resolve(GALLERY, `01-rail/determinism-${suffix}.png`);
    await page.screenshot({ path });
    return path;
  };
  const a = await capture("a");
  const b = await capture("b");
  const { readFileSync, statSync, unlinkSync } = await import("node:fs");
  const sizeA = statSync(a).size;
  const sizeB = statSync(b).size;
  // Byte comparison (pinned clock + reduced motion + fixed viewport).
  const identical = readFileSync(a).equals(readFileSync(b));
  if (identical) {
    unlinkSync(b); // keep one artifact
  } else {
    // PNG streams may differ in chunk ordering; report honestly rather
    // than fake a pass — the task requires 0 px diff, so fail loudly.
    throw new Error(
      `determinism check failed: captures differ (sizes ${sizeA} vs ${sizeB}); see determinism-a/b.png`,
    );
  }
  if (existsSync(resolve(GALLERY, "determinism-b.png"))) {
    // already unlinked on success; no-op
  }
});


// AJ-E505 port-map captures: the inventory with all four
// classifications (unmanaged muted, icon+text — never colour alone)
// and the pre-launch collision sheet (copy-exact), both themes, two
// sizes.
const PORTS_FIXTURE = {
  rows: [
    {
      port: 3000, pid: 4400, class: "managed-running", session: "s-2", provider: "claude-code",
      account: "Team Max", project: "api", listener_age_ms: 65_000, name: "node", hint: "common dev server port", killable: true,
    },
    {
      port: 5173, pid: 8123, class: "managed-orphan", session: "s-1", provider: "claude-code",
      account: "Work Max", project: "web", listener_age_ms: 9_000_000, name: "python3", hint: "vite dev server port", killable: true,
    },
    {
      port: 8080, pid: 5500, class: "re-adopted", session: "s-3", provider: "claude-code",
      account: null, project: null, listener_age_ms: 250_000, name: "python3", hint: "common dev server port", killable: true,
    },
    {
      port: 8000, pid: 6600, class: "not-managed", session: null, provider: null,
      account: null, project: null, listener_age_ms: null, name: null, hint: "common dev server port", killable: false,
    },
  ],
  note: null,
  hogs: [],
};

for (const theme of THEMES) {
  for (const vp of [
    { name: "1440x900", width: 1440, height: 900 },
    { name: "1024x700", width: 1024, height: 700 },
  ]) {
    test(`ports — inventory, four classifications — ${vp.name} — ${theme}`, async ({ page }) => {
      await page.addInitScript(
        ({ fixture, t0, ports, theme }) => {
          (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
          (window as never as { __AIJ_PORTS__: unknown }).__AIJ_PORTS__ = ports;
          localStorage.setItem("aij.theme", theme);
          localStorage.setItem("aij.onboarding.dismissed", "1");
          sessionStorage.setItem("aij.ready.skipped", "1");
          Date.now = () => t0;
        },
        { fixture: allStatuses, t0: T0, ports: PORTS_FIXTURE, theme },
      );
      await page.emulateMedia({ reducedMotion: "reduce" });
      await page.setViewportSize({ width: vp.width, height: vp.height });
      await page.goto("/");
      await page.getByRole("tab", { name: "Sessions" }).click();
      await page.getByRole("button", { name: /Work Max/ }).first().click();
      await page.getByRole("tab", { name: "Processes" }).click();
      await expect(page.getByTestId("port-inventory")).toBeVisible();
      await expect(page.getByTestId("port-row-5173")).toContainText("Managed orphan");
      await expect(page.getByTestId("port-row-8000")).toContainText("Not managed by Juggler");
      await page.screenshot({ path: resolve(GALLERY, `18-ports/inventory-${vp.name}-${theme}.png`) });
    });
  }
}

for (const theme of THEMES) {
  test(`ports — pre-launch collision sheet — 1440x900 — ${theme}`, async ({ page }) => {
    await page.addInitScript(
      ({ fixture, t0, warning, theme }) => {
        (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
        (window as never as { __AIJ_PORT_WARNING__: unknown }).__AIJ_PORT_WARNING__ = warning;
        localStorage.setItem("aij.theme", theme);
        localStorage.setItem("aij.onboarding.dismissed", "1");
        sessionStorage.setItem("aij.ready.skipped", "1");
        Date.now = () => t0;
      },
      {
        fixture: allStatuses,
        t0: T0,
        theme,
        warning: {
          port: 5173,
          pid: 8123,
          instance: "claude-code (s-1a2b3c4d)",
          age_ms: 9_000_000,
          message: ":5173 is already held by an orphan from claude-code (s-1a2b3c4d) (2h).",
        },
      },
    );
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto("/");
    // Trigger a launch through the Instance Actions' Resume row —
    // the pre-check returns the fixture warning and the sheet renders
    // instead of spawning (launch is never blocked; the sheet offers
    // Launch anyway).
    await page.getByRole("tab", { name: "Sessions" }).click();
    await page.getByRole("button", { name: /Work Max/ }).first().click();
    await page.getByRole("tab", { name: "Actions" }).click();
    await page.getByRole("button", { name: /Resume|Relaunch/ }).first().click();
    await expect(page.getByTestId("port-warning-sheet")).toBeVisible();
    await expect(page.getByTestId("port-warning-copy")).toContainText(
      ":5173 is already held by an orphan",
    );
    await page.screenshot({ path: resolve(GALLERY, `18-ports/warning-sheet-1440x900-${theme}.png`) });
  });
}


// AJ-E506 Cleanup Centre captures: six states (needs-attention with
// orphan+hog+collision, all-managed, re-adopted, unverifiable,
// empty-success, ledger-degraded) × themes × sizes.
const CLEANUP_ROW = (pid: number, state: string) => ({
  pid,
  pgid: 8100,
  role: "helper",
  state,
  state_label:
    state === "orphaned"
      ? "Still running after its Instance closed"
      : state === "adopted"
        ? "Running — re-adopted"
        : state === "unverifiable"
          ? "Unverifiable — will not be touched"
          : "Running",
  role_guess: "python3 (guess)",
  exe_path: "/usr/bin/python3",
  cmdline: "python3 -m http.server 5173",
  age_ms: 9_000_000,
  rss_bytes: 48 * 1024 * 1024,
  cpu_pct: 1.4,
  ports: state === "orphaned" ? "5173" : null,
  hog: pid === 8124,
  hog_rules: pid === 8124 ? ["tree-rss>p95(provider)"] : [],
  evidence: {
    root_pid: 8100,
    parent_pid: 8100,
    pgid: 8100,
    escaped_group: false,
    verification: {
      verdict: state === "unverifiable" ? "unverifiable" : "verified",
      detail:
        state === "unverifiable"
          ? "live start-time or exe differs from the record"
          : "start-time and exe match the live process",
    },
    first_seen: T0,
    last_seen: T0 + 90_000,
  },
});

function cleanupFixture(state: string) {
  if (state === "empty") {
    return { groups: [], clean_instances: 3, ledger_degraded: false };
  }
  if (state === "degraded") {
    return { groups: [], clean_instances: 0, ledger_degraded: true };
  }
  // The FILTER states map to row states: needs-attention shows the
  // orphan + the hog; re-adopted shows the adopted row; unverifiable
  // shows the flagged one; all-managed shows the mix.
  const firstState =
    state === "needs-attention"
      ? "orphaned"
      : state === "re-adopted"
        ? "adopted"
        : "running";
  const rows = [
    CLEANUP_ROW(8123, firstState),
    CLEANUP_ROW(8124, state === "unverifiable" ? "unverifiable" : "running"),
  ];
  return {
    groups: [
      {
        session: "s-1",
        provider: "claude-code",
        account: "Work Max",
        project: "web",
        machine: "machine-local",
        healthy: false,
        rows,
      },
    ],
    clean_instances: 2,
    ledger_degraded: false,
  };
}

for (const state of ["needs-attention", "all-managed", "re-adopted", "unverifiable", "empty", "degraded"]) {
  for (const theme of THEMES) {
    for (const vp of [
      { name: "1440x900", width: 1440, height: 900 },
      { name: "1024x700", width: 1024, height: 700 },
    ]) {
      test(`cleanup — ${state} — ${vp.name} — ${theme}`, async ({ page }) => {
        await page.addInitScript(
          ({ fixture, t0, cleanup, theme }) => {
            (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
            (window as never as { __AIJ_CLEANUP__: unknown }).__AIJ_CLEANUP__ = cleanup;
            localStorage.setItem("aij.theme", theme);
            localStorage.setItem("aij.onboarding.dismissed", "1");
            sessionStorage.setItem("aij.ready.skipped", "1");
            Date.now = () => t0;
          },
          { fixture: allStatuses, t0: T0, cleanup: cleanupFixture(state), theme },
        );
        await page.emulateMedia({ reducedMotion: "reduce" });
        await page.setViewportSize({ width: vp.width, height: vp.height });
        await page.goto("/");
        // Reached from the TOP-RIGHT global menus (ADR-0007) — never
        // a rail peer.
        await page.getByRole("button", { name: "Cleanup" }).click();
        await expect(page.getByTestId("cleanup-centre")).toBeVisible();
        if (state === "needs-attention") {
          await expect(page.getByTestId("cleanup-row-8123")).toContainText(
            "Still running after its Instance closed",
          );
          await expect(page.getByTestId("cleanup-row-8124")).toContainText("hog");
        }
        if (state === "empty") {
          await expect(page.getByTestId("cleanup-empty")).toContainText(
            "Nothing left behind — 3 Instances running clean.",
          );
        }
        if (state === "degraded") {
          await expect(page.getByTestId("cleanup-degraded-banner")).toBeVisible();
        }
        if (state === "unverifiable") {
          await expect(page.getByTestId("cleanup-state-8124")).toContainText(
            "Unverifiable — will not be touched",
          );
        }
        await page.screenshot({ path: resolve(GALLERY, `19-cleanup/${state}-${vp.name}-${theme}.png`) });
      });
    }
  }
}


// AJ-E509 history captures: audit rows (manual + automatic actors,
// outcome vocabulary incl. the crash-pending unknown) and the
// empty-history copy — via the Cleanup Centre's History filter.
for (const theme of THEMES) {
  test(`cleanup — history populated — 1440x900 — ${theme}`, async ({ page }) => {
    await page.addInitScript(
      ({ fixture, t0, cleanup, theme }) => {
        (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
        (window as never as { __AIJ_CLEANUP__: unknown }).__AIJ_CLEANUP__ = cleanup;
        localStorage.setItem("aij.theme", theme);
        localStorage.setItem("aij.onboarding.dismissed", "1");
        sessionStorage.setItem("aij.ready.skipped", "1");
        Date.now = () => t0;
      },
      {
        fixture: allStatuses,
        t0: T0,
        theme,
        cleanup: {
          groups: [],
          clean_instances: 0,
          ledger_degraded: false,
          history: {
            rows: [
              {
                id: 1, at_ms: T0 - 90_000, actor: "user:orphan_stop", automatic: false,
                pid: 8123, cmdline_trunc: "python3 -m http.server 5173", session_id: "s-1",
                provider: "claude-code", account_id: "Work Max", machine_id: "machine-local",
                outcome: "exited on TERM", ports_before: "5173", ports_after: "",
              },
              {
                id: 2, at_ms: T0 - 200_000, actor: "auto:tier-1", automatic: true,
                pid: 4400, cmdline_trunc: "node vite", session_id: "s-2",
                provider: "claude-code", account_id: null, machine_id: "machine-local",
                outcome: "TERM'd, outcome unknown", ports_before: null, ports_after: null,
              },
            ],
            empty_note: null,
          },
        },
      },
    );
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto("/");
    await page.getByRole("button", { name: "Cleanup" }).click();
    await page.getByRole("radio", { name: "History (7 days)" }).click();
    await expect(page.getByTestId("cleanup-history")).toBeVisible();
    await expect(page.getByTestId("cleanup-history-actor-2")).toContainText("automatic");
    await page.screenshot({ path: resolve(GALLERY, `19-cleanup/history-populated-1440x900-${theme}.png`) });
  });

  test(`cleanup — history empty — 1440x900 — ${theme}`, async ({ page }) => {
    await page.addInitScript(
      ({ fixture, t0, cleanup, theme }) => {
        (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
        (window as never as { __AIJ_CLEANUP__: unknown }).__AIJ_CLEANUP__ = cleanup;
        localStorage.setItem("aij.theme", theme);
        localStorage.setItem("aij.onboarding.dismissed", "1");
        sessionStorage.setItem("aij.ready.skipped", "1");
        Date.now = () => t0;
      },
      {
        fixture: allStatuses,
        t0: T0,
        theme,
        cleanup: {
          groups: [],
          clean_instances: 0,
          ledger_degraded: false,
          history: {
            rows: [],
            empty_note: "No cleanups yet — automatic cleanup is off by default.",
          },
        },
      },
    );
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto("/");
    await page.getByRole("button", { name: "Cleanup" }).click();
    await page.getByRole("radio", { name: "History (7 days)" }).click();
    await expect(page.getByTestId("cleanup-history-empty")).toContainText(
      "No cleanups yet — automatic cleanup is off by default.",
    );
    await page.screenshot({ path: resolve(GALLERY, `19-cleanup/history-empty-1440x900-${theme}.png`) });
  });
}


// ---------------------------------------------------------------------------
// AJ-E507 — the verify-before-kill sheet: the three steps and every
// state. All states ride the SAME needs-attention fixture (verified
// orphan row holding :5173); the sheet is opened the real way — the
// row's Terminate… affordance — never mounted out of context.
// ---------------------------------------------------------------------------

const KILL_VERIFY_OK = {
  pid: 8123,
  session: "s-1",
  verified: true,
  refusal: null,
  exe_path: "/usr/local/bin/node",
  cmdline: "node /srv/web/node_modules/.bin/vite dev",
  cwd: "/srv/web",
  ports: [5173],
  pgid: 8100,
  escaped_group: false,
  members: [
    {
      pid: 8100,
      pgid: 8100,
      exe: "sh",
      cmdline: "sh -c vite dev",
      role_guess: "TerminalShell",
      unsaved_state_warning: "Includes a TerminalShell that may hold unsaved state.",
      from_ledger: true,
    },
    {
      pid: 8123,
      pgid: 8100,
      exe: "node",
      cmdline: "node vite dev",
      role_guess: "DevServer",
      unsaved_state_warning: null,
      from_ledger: true,
    },
  ],
  tree_pids: [8100, 8123],
  platform: "TERM → grace → KILL",
  grace_secs: 5,
  provider: "claude-code",
  account_id: "Work Max",
  project_id: "web",
};

const KILL_VERIFY_FAILED = {
  ...KILL_VERIFY_OK,
  verified: false,
  refusal: "This PID now belongs to a different program — Juggler will not touch it.",
  members: [],
};

const KILL_EXECUTE_REPORT = {
  refusal: null,
  members: [
    { pid: 8100, outcome: "already gone" },
    { pid: 8123, outcome: "needed KILL" },
  ],
  cancelled: false,
  all_succeeded: true,
  still_holding: [{ port: 5173, pid: 9100, exe: "node" }],
  reoffer_pid: 9100,
};

const KILL_EXECUTE_FREED = {
  refusal: null,
  members: [
    { pid: 8123, outcome: "exited on TERM" },
  ],
  cancelled: false,
  all_succeeded: true,
  still_holding: [],
  reoffer_pid: null,
};

const killSeams = (state: string) => ({
  verify:
    state === "verification-failed"
      ? KILL_VERIFY_FAILED
      : state === "windows-wording"
        ? { ...KILL_VERIFY_OK, platform: "TerminateProcess only", grace_secs: 0 }
        : KILL_VERIFY_OK,
  pending: state === "grace-countdown",
  execute: state === "outcome-report" ? KILL_EXECUTE_REPORT : KILL_EXECUTE_FREED,
});

for (const state of [
  "verify-pid",
  "verify-pgid",
  "verify-tree",
  "verification-failed",
  "grace-countdown",
  "outcome-report",
  "outcome-freed",
  "windows-wording",
] as const) {
  for (const theme of THEMES) {
    for (const vp of [
      { name: "1440x900", width: 1440, height: 900 },
      { name: "1024x700", width: 1024, height: 700 },
    ]) {
      test(`kill-sheet — ${state} — ${vp.name} — ${theme}`, async ({ page }) => {
        const seams = killSeams(state);
        await page.addInitScript(
          ({ fixture, t0, cleanup, theme, seams }) => {
            (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
            (window as never as { __AIJ_CLEANUP__: unknown }).__AIJ_CLEANUP__ = cleanup;
            const w = window as never as Record<string, unknown>;
            w.__AIJ_KILL_VERIFY__ = () => seams.verify;
            // The countdown state leaves execute UNSET so the pending
            // seam applies (a ladder that never resolves); every
            // other state resolves with the report fixture.
            if (!seams.pending) {
              w.__AIJ_KILL_EXECUTE__ = () => seams.execute;
            }
            w.__AIJ_KILL_EXECUTE_PENDING__ = seams.pending;
            // During the countdown the members are still alive (the
            // per-process ticks show "running" → "exited" honestly).
            w.__AIJ_KILL_LIVENESS__ = (pids: number[]) => pids.map(() => true);
            localStorage.setItem("aij.theme", theme);
            localStorage.setItem("aij.onboarding.dismissed", "1");
            sessionStorage.setItem("aij.ready.skipped", "1");
            Date.now = () => t0;
          },
          { fixture: allStatuses, t0: T0, cleanup: cleanupFixture("needs-attention"), theme, seams },
        );
        await page.emulateMedia({ reducedMotion: "reduce" });
        await page.setViewportSize({ width: vp.width, height: vp.height });
        await page.goto("/");
        await page.getByRole("button", { name: "Cleanup" }).click();
        await expect(page.getByTestId("cleanup-centre")).toBeVisible();
        // The REAL entry: the verified orphan row's affordance.
        await page.getByTestId("cleanup-kill-8123").click();
        await expect(page.getByTestId("kill-sheet-8123")).toBeVisible();

        if (state === "verification-failed") {
          await expect(page.getByTestId("kill-refused-8123")).toContainText(
            "This PID now belongs to a different program",
          );
          // No kill button in the failed-state DOM (Req.2 gate).
          const confirm = await page.getByTestId("kill-confirm-8123").count();
          if (confirm !== 0) throw new Error("kill button present in failed-state DOM");
        } else if (state === "verify-pgid" || state === "verify-tree") {
          await expect(page.getByTestId("kill-ready-8123")).toBeVisible();
          await page
            .getByTestId(`kill-scope-${state === "verify-pgid" ? "pgid" : "tree"}-8123`)
            .check();
          await expect(page.getByTestId("kill-member-8123")).toBeVisible();
        } else if (state === "grace-countdown") {
          await expect(page.getByTestId("kill-ready-8123")).toBeVisible();
          await page.getByTestId("kill-confirm-8123").click();
          // The spec's "grace countdown at 3 s" moment.
          await expect(page.getByTestId("kill-countdown-8123")).toContainText("3s", {
            timeout: 8_000,
          });
          await expect(page.getByTestId("kill-tick-8123")).toContainText("running");
        } else if (state === "outcome-report" || state === "outcome-freed") {
          await expect(page.getByTestId("kill-ready-8123")).toBeVisible();
          await page.getByTestId("kill-confirm-8123").click();
          await expect(page.getByTestId("kill-report-8123")).toBeVisible();
          if (state === "outcome-report") {
            await expect(page.getByTestId("kill-still-holding-8123")).toContainText(":5173");
          } else {
            await expect(page.getByTestId("kill-success-8123")).toContainText(":5173 is free.");
          }
        } else if (state === "windows-wording") {
          await expect(page.getByTestId("kill-platform-8123")).toContainText(
            "Windows: TerminateProcess only",
          );
        } else {
          await expect(page.getByTestId("kill-ready-8123")).toBeVisible();
          await expect(page.getByTestId("kill-confirm-8123")).toContainText(
            "Terminate vite (PID 8123) and free :5173",
          );
        }
        await page.screenshot({
          path: resolve(GALLERY, `20-kill/${state}-${vp.name}-${theme}.png`),
        });
      });
    }
  }
}


// ---------------------------------------------------------------------------
// AJ-E508 — the tier surfaces: the per-Account toggles (off/on) in the
// Account launch-settings area, the Cleanup Centre's paused banner,
// the suspended-after-crash banner, and the Tier-2 notification with
// its Undo window (at appearance and at 5 s remaining).
// ---------------------------------------------------------------------------

async function seedTiersPage(
  page: import("@playwright/test").Page,
  vp: { width: number; height: number },
  theme: string,
  seams: Record<string, unknown>,
) {
  await page.addInitScript(
    ({ fixture, t0, theme: t, seams: s }) => {
      const w = window as unknown as Record<string, unknown>;
      w.__AIJ_FIXTURE__ = fixture;
      for (const [key, value] of Object.entries(s)) {
        w[key] = value;
      }
      localStorage.setItem("aij.theme", t);
      localStorage.setItem("aij.onboarding.dismissed", "1");
      sessionStorage.setItem("aij.ready.skipped", "1");
      Date.now = () => t0;
    },
    { fixture: allStatuses, t0: T0, theme, seams },
  );
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.setViewportSize({ width: vp.width, height: vp.height });
  await page.goto("/");
}

for (const theme of THEMES) {
  for (const vp of [{ name: "1440x900", width: 1440, height: 900 }]) {
    test(`tiers — account toggles off — ${vp.name} — ${theme}`, async ({ page }) => {
      await seedTiersPage(page, vp, theme, {
        __AIJ_CLEANUP_TIERS__: { tier1_opt_in: false, tier2_opt_in: false },
      });
      await openAccountOverview(page);
      await expect(page.getByTestId("account-tier-toggles")).toBeVisible();
      await expect(page.getByTestId("account-cleanup-toggle")).toContainText(
        "Automatically tidy left-behind dev servers",
      );
      await expect(page.getByTestId("account-tier2-toggle")).toContainText(
        "Also tidy after a crash (with notification)",
      );
      await page.screenshot({
        path: resolve(GALLERY, `21-tiers/toggles-off-${vp.name}-${theme}.png`),
      });
    });

    test(`tiers — account toggles on — ${vp.name} — ${theme}`, async ({ page }) => {
      await seedTiersPage(page, vp, theme, {
        __AIJ_CLEANUP_TIERS__: { tier1_opt_in: true, tier2_opt_in: true },
      });
      await openAccountOverview(page);
      await expect(page.getByTestId("account-tier-toggles")).toBeVisible();
      await expect(
        page.getByTestId("account-cleanup-toggle").locator("input"),
      ).toBeChecked();
      await expect(
        page.getByTestId("account-tier2-toggle").locator("input"),
      ).toBeChecked();
      await page.screenshot({
        path: resolve(GALLERY, `21-tiers/toggles-on-${vp.name}-${theme}.png`),
      });
    });

    test(`tiers — centre paused banner — ${vp.name} — ${theme}`, async ({ page }) => {
      await seedTiersPage(page, vp, theme, {
        __AIJ_CLEANUP_STATUS__: {
          paused: true,
          suspended: false,
          suspended_until_ms: null,
          tier2_pending: [],
        },
      });
      await page.getByRole("button", { name: "Cleanup" }).click();
      await expect(page.getByTestId("cleanup-paused-banner")).toContainText(
        "Automatic cleanup is paused.",
      );
      await expect(page.getByTestId("cleanup-pause-toggle")).toContainText("Resume");
      await page.screenshot({
        path: resolve(GALLERY, `21-tiers/paused-${vp.name}-${theme}.png`),
      });
    });

    test(`tiers — suspended after crash — ${vp.name} — ${theme}`, async ({ page }) => {
      await seedTiersPage(page, vp, theme, {
        __AIJ_CLEANUP_STATUS__: {
          paused: false,
          suspended: true,
          suspended_until_ms: T0 + 24 * 3_600_000,
          tier2_pending: [],
        },
      });
      await page.getByRole("button", { name: "Cleanup" }).click();
      await expect(page.getByTestId("cleanup-suspended-banner")).toContainText(
        "suspended for 24 h",
      );
      await page.screenshot({
        path: resolve(GALLERY, `21-tiers/suspended-${vp.name}-${theme}.png`),
      });
    });

    test(`tiers — tier-2 notification at appearance — ${vp.name} — ${theme}`, async ({ page }) => {
      await seedTiersPage(page, vp, theme, {
        __AIJ_CLEANUP_STATUS__: {
          paused: false,
          suspended: false,
          suspended_until_ms: null,
          tier2_pending: [
            {
              pid: 8123,
              session: "s-1",
              cmdline_trunc: "node vite dev",
              deadline_ms: T0 + 30_000,
            },
          ],
        },
      });
      await page.getByRole("button", { name: "Cleanup" }).click();
      await expect(page.getByTestId("cleanup-tier2-notice-8123")).toContainText(
        "survived a crash",
      );
      await expect(page.getByTestId("cleanup-tier2-notice-8123")).toContainText("in 30s");
      await page.screenshot({
        path: resolve(GALLERY, `21-tiers/tier2-appearance-${vp.name}-${theme}.png`),
      });
    });

    test(`tiers — tier-2 notification at 5 s remaining — ${vp.name} — ${theme}`, async ({ page }) => {
      await seedTiersPage(page, vp, theme, {
        __AIJ_CLEANUP_STATUS__: {
          paused: false,
          suspended: false,
          suspended_until_ms: null,
          tier2_pending: [
            {
              pid: 8123,
              session: "s-1",
              cmdline_trunc: "node vite dev",
              deadline_ms: T0 + 5_000,
            },
          ],
        },
      });
      await page.getByRole("button", { name: "Cleanup" }).click();
      await expect(page.getByTestId("cleanup-tier2-notice-8123")).toContainText("in 5s");
      await expect(page.getByTestId("cleanup-tier2-undo-8123")).toContainText("Undo");
      await page.screenshot({
        path: resolve(GALLERY, `21-tiers/tier2-5s-${vp.name}-${theme}.png`),
      });
    });
  }
}


// ---------------------------------------------------------------------------
// AJ-E510 — crash-recovery captures: the ONE-time restart summary (the
// exact 2/1/1 drill text rides the __AIJ_RECOVERY__ seam), the
// re-adopted Instance panel (the ↻ chip + the honest detached-terminal
// notice on the Processes tab), and the Cleanup Centre's Re-adopted
// filter with the distinct left-behind label.
// ---------------------------------------------------------------------------

const RECOVERY_SUMMARY = {
  summary:
    "2 Instances re-adopted, 1 helpers left behind, 1 processes could not be verified and were left alone.",
  adopted: 2,
  left_behind: 1,
  unverifiable: 1,
};

const RECOVERY_CLEANUP = {
  groups: [
    {
      session: "s-crash",
      provider: "claude-code",
      account: "Work Max",
      project: "web",
      machine: "machine-local",
      healthy: false,
      rows: [
        {
          pid: 8402, pgid: 8400, role: "root", state: "adopted",
          state_label: "Running — re-adopted", down_period: false,
          role_guess: "sh (guess)", exe_path: "/bin/sh", cmdline: "sh -c claude",
          age_ms: 45 * 60_000, rss_bytes: 12_000_000, cpu_pct: 0.2, ports: null,
          hog: false, hog_rules: [],
          evidence: {
            root_pid: 8402, parent_pid: null, pgid: 8400, escaped_group: false,
            verification: { verdict: "verified", detail: "start-time matches the live process" },
            first_seen: T0 - 45 * 60_000, last_seen: T0,
          },
        },
        {
          pid: 8301, pgid: 8300, role: "helper", state: "orphaned",
          state_label: "left behind while Juggler was closed", down_period: true,
          role_guess: "python3 (guess)", exe_path: "/usr/bin/python3",
          cmdline: "python3 -m http.server 5173",
          age_ms: 9_000_000, rss_bytes: 48 * 1024 * 1024, cpu_pct: 1.4, ports: "5173",
          hog: false, hog_rules: [],
          evidence: {
            root_pid: 8300, parent_pid: 8300, pgid: 8300, escaped_group: false,
            verification: { verdict: "verified", detail: "start-time and exe match the live process" },
            first_seen: T0 - 9_000_000, last_seen: T0,
          },
        },
      ],
    },
  ],
  clean_instances: 1,
  ledger_degraded: false,
};

async function seedRecoveryPage(
  page: import("@playwright/test").Page,
  theme: string,
  seams: Record<string, unknown>,
) {
  await page.addInitScript(
    ({ fixture, t0, theme: t, seams: s }) => {
      const w = window as unknown as Record<string, unknown>;
      w.__AIJ_FIXTURE__ = fixture;
      for (const [key, value] of Object.entries(s)) {
        w[key] = value;
      }
      localStorage.setItem("aij.theme", t);
      localStorage.setItem("aij.onboarding.dismissed", "1");
      sessionStorage.setItem("aij.ready.skipped", "1");
      Date.now = () => t0;
    },
    { fixture: allStatuses, t0: T0, theme, seams },
  );
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/");
}

for (const theme of THEMES) {
  test(`recovery — one-time summary dialog — 1440x900 — ${theme}`, async ({ page }) => {
    await seedRecoveryPage(page, theme, { __AIJ_RECOVERY__: RECOVERY_SUMMARY });
    const dialog = page.getByTestId("recovery-summary");
    await expect(dialog).toBeVisible();
    // The exact drill text (one source of truth with the Rust test).
    await expect(page.getByTestId("recovery-summary-text")).toHaveText(RECOVERY_SUMMARY.summary);
    await expect(dialog).toContainText("stays suspended for 24 hours");
    await page.screenshot({
      path: resolve(GALLERY, `22-recovery/summary-dialog-1440x900-${theme}.png`),
    });
  });

  test(`recovery — adopted panel: chip + detached notice — 1440x900 — ${theme}`, async ({ page }) => {
    // sess-2 (the running Work Max terminal) survived the crash and was
    // re-adopted at restart.
    await page.addInitScript(
      ({ fixture, t0, theme: t }) => {
        const w = window as unknown as Record<string, unknown>;
        w.__AIJ_FIXTURE__ = {
          ...fixture,
          sessions: fixture.sessions.map((session: { id: string }) =>
            session.id === "sess-2" ? { ...session, adopted: true } : session,
          ),
        };
        localStorage.setItem("aij.theme", t);
        localStorage.setItem("aij.onboarding.dismissed", "1");
        sessionStorage.setItem("aij.ready.skipped", "1");
        Date.now = () => t0;
      },
      { fixture: allStatuses, t0: T0, theme },
    );
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto("/");
    await page.getByRole("tab", { name: "Sessions" }).click();
    await page
      .getByRole("button", { name: /Work Max/ })
      .filter({ hasText: /running/i })
      .first()
      .click();
    await expect(page.getByTestId("instance-adopted-chip")).toContainText("↻ re-adopted");
    await page.getByRole("tab", { name: "Processes" }).click();
    const notice = page.getByTestId("instance-detached-notice");
    await expect(notice).toBeVisible();
    await expect(notice).toContainText("Running — re-adopted after restart");
    await expect(notice).toContainText("terminal is detached");
    await page.screenshot({
      path: resolve(GALLERY, `22-recovery/adopted-panel-1440x900-${theme}.png`),
    });
  });

  test(`recovery — cleanup Re-adopted filter + left-behind label — 1440x900 — ${theme}`, async ({ page }) => {
    await seedRecoveryPage(page, theme, { __AIJ_CLEANUP__: RECOVERY_CLEANUP });
    await page.getByRole("button", { name: "Cleanup" }).click();
    await expect(page.getByTestId("cleanup-centre")).toBeVisible();
    await page.getByRole("radio", { name: "Re-adopted" }).click();
    await expect(page.getByTestId("cleanup-state-8402")).toContainText("Running — re-adopted");
    await expect(page.getByTestId("cleanup-state-8301")).toContainText(
      "left behind while Juggler was closed",
    );
    await page.screenshot({
      path: resolve(GALLERY, `22-recovery/cleanup-readopted-1440x900-${theme}.png`),
    });
  });
}


// ---------------------------------------------------------------------------
// AJ-E512 — the passive alert surfaces: the hog alert (exact copy +
// staged sheet pre-fill) and the idle-orphan nudge, plus the recorded
// keyboard-only pass (open Centre → review hog → stage sheet → Esc).
// ---------------------------------------------------------------------------

const HOG_ALERT = {
  kind: "hog",
  text: "node (vite) under Work Max holds 3.1 GiB — review in Cleanup Centre.",
  pid: 8123,
  session: "s-1",
};
const IDLE_ALERT = {
  kind: "idle",
  text: "python3 has been idle for 2h — review it in the Cleanup Centre.",
  pid: 8124,
  session: "s-1",
};

for (const theme of THEMES) {
  for (const vp of [
    { name: "1440x900", width: 1440, height: 900 },
    { name: "1024x700", width: 1024, height: 700 },
  ]) {
    test(`alerts — hog alert + staged sheet — ${vp.name} — ${theme}`, async ({ page }) => {
      await seedRecoveryPage(page, theme, {
        __AIJ_CLEANUP__: cleanupFixture("needs-attention"),
        __AIJ_CLEANUP_ALERTS__: { alerts: [HOG_ALERT] },
        __AIJ_KILL_VERIFY__: () => ({
          pid: 8123,
          session: "s-1",
          verified: true,
          refusal: null,
          exe_path: "/usr/local/bin/node",
          cmdline: "node vite dev",
          cwd: "/srv/web",
          ports: [5173],
          pgid: 8100,
          escaped_group: false,
          members: [
            {
              pid: 8123, pgid: 8100, exe: "node", cmdline: "node vite dev",
              role_guess: "DevServer", unsaved_state_warning: null, from_ledger: true,
            },
          ],
          tree_pids: [8100, 8123],
          platform: "TERM → grace → KILL",
          grace_secs: 5,
          provider: "claude-code",
          account_id: "Work Max",
          project_id: "web",
        }),
        __AIJ_KILL_EXECUTE__: () => ({
          refusal: null,
          members: [{ pid: 8123, outcome: "exited on TERM" }],
          cancelled: false,
          all_succeeded: true,
          still_holding: [],
          reoffer_pid: null,
        }),
      });
      await page.getByRole("button", { name: "Cleanup" }).click();
      await expect(page.getByTestId("cleanup-alerts")).toBeVisible();
      await expect(page.getByTestId("hog-alert-8123")).toContainText(
        "node (vite) under Work Max holds 3.1 GiB — review in Cleanup Centre.",
      );
      // A status message, not a dialog.
      await expect(
        page.getByRole("status", { name: "Passive cleanup alerts" }),
      ).toHaveAttribute("aria-live", "polite");
      await page.getByTestId("hog-stage-8123").click();
      await expect(page.getByTestId("kill-sheet-8123")).toBeVisible();
      await page.screenshot({ path: resolve(GALLERY, `23-alerts/hog-staged-${vp.name}-${theme}.png`) });
    });

    test(`alerts — idle nudge (passive, no action) — ${vp.name} — ${theme}`, async ({ page }) => {
      await seedRecoveryPage(page, theme, {
        __AIJ_CLEANUP__: cleanupFixture("needs-attention"),
        __AIJ_CLEANUP_ALERTS__: { alerts: [IDLE_ALERT] },
      });
      await page.getByRole("button", { name: "Cleanup" }).click();
      await expect(page.getByTestId("idle-nudge-8124")).toBeVisible();
      await expect(page.getByTestId("idle-nudge-8124")).toContainText(
        "has been idle for 2h",
      );
      await expect(page.getByTestId("idle-nudge-8124").locator("button")).toHaveCount(0);
      await page.screenshot({ path: resolve(GALLERY, `23-alerts/idle-nudge-${vp.name}-${theme}.png`) });
    });

    test(`alerts — mixed gate-day state — ${vp.name} — ${theme}`, async ({ page }) => {
      await seedRecoveryPage(page, theme, {
        __AIJ_CLEANUP__: cleanupFixture("needs-attention"),
        __AIJ_CLEANUP_ALERTS__: { alerts: [HOG_ALERT, IDLE_ALERT] },
      });
      await page.getByRole("button", { name: "Cleanup" }).click();
      await expect(page.getByTestId("hog-alert-8123")).toBeVisible();
      await expect(page.getByTestId("idle-nudge-8124")).toBeVisible();
      await expect(page.getByTestId("cleanup-row-8124")).toContainText("hog");
      await page.screenshot({ path: resolve(GALLERY, `23-alerts/mixed-${vp.name}-${theme}.png`) });
    });
  }
}

// The recorded keyboard-only pass (AJ-E512 visual testing): open the
// Centre → Tab to the hog alert's stage button → Enter opens the
// verified sheet → Esc closes it → focus RETURNS to the stage button
// (2.4.3). No pointer input anywhere in the pass.
test("alerts — keyboard-only pass: review hog, stage sheet, Esc returns focus", async ({ page }) => {
  await seedRecoveryPage(page, "dark", {
    __AIJ_CLEANUP__: cleanupFixture("needs-attention"),
    __AIJ_CLEANUP_ALERTS__: { alerts: [HOG_ALERT] },
    __AIJ_KILL_VERIFY__: () => ({
      pid: 8123, session: "s-1", verified: true, refusal: null,
      exe_path: "/usr/local/bin/node", cmdline: "node vite dev", cwd: "/srv/web",
      ports: [5173], pgid: 8100, escaped_group: false,
      members: [
        { pid: 8123, pgid: 8100, exe: "node", cmdline: "node vite dev",
          role_guess: "DevServer", unsaved_state_warning: null, from_ledger: true },
      ],
      tree_pids: [8100, 8123], platform: "TERM → grace → KILL", grace_secs: 5,
      provider: "claude-code", account_id: "Work Max", project_id: "web",
    }),
  });
  await page.getByRole("button", { name: "Cleanup" }).click();
  await expect(page.getByTestId("hog-alert-8123")).toBeVisible();
  const stage = page.getByTestId("hog-stage-8123");
  // Keyboard-only traversal: Tab until the stage button owns focus.
  for (let i = 0; i < 30; i += 1) {
    const focused = await page.evaluate(
      () => document.activeElement?.getAttribute("data-testid") ?? "",
    );
    if (focused === "hog-stage-8123") break;
    await page.keyboard.press("Tab");
  }
  await expect(stage).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByTestId("kill-sheet-8123")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("kill-sheet-8123")).toHaveCount(0);
  await expect(stage).toBeFocused();
  await page.screenshot({ path: resolve(GALLERY, "23-alerts/keyboard-pass-1440x900-dark.png") });
});


// ---------------------------------------------------------------------------
// AJ-E511 — the fleet machine dimension: the quiet Machine column
// (This Mac locally, the machine id on foreign rows), the labelled
// machine filter (default This machine), and the display-only
// foreign-row state (no kill control in the DOM).
// ---------------------------------------------------------------------------

const FLEET_CLEANUP = {
  groups: [
    {
      session: "s-local",
      provider: "claude-code",
      account: "Work Max",
      project: "web",
      machine: "machine-local",
      healthy: false,
      rows: [
        {
          machine_id: "machine-local",
          pid: 8123, pgid: 8100, role: "helper", state: "orphaned",
          state_label: "Still running after its Instance closed",
          role_guess: "python3 (guess)", exe_path: "/usr/bin/python3",
          cmdline: "python3 -m http.server 5173", age_ms: 9_000_000,
          rss_bytes: 48 * 1024 * 1024, cpu_pct: 1.4, ports: "5173",
          hog: false, hog_rules: [],
          evidence: {
            root_pid: 8100, parent_pid: 8100, pgid: 8100, escaped_group: false,
            verification: { verdict: "verified", detail: "match" },
            first_seen: T0, last_seen: T0 + 90_000,
          },
        },
      ],
    },
    {
      session: "s-local",
      provider: "claude-code",
      account: "Work Max",
      project: "web",
      machine: "machine-studio",
      healthy: false,
      rows: [
        {
          machine_id: "machine-studio",
          pid: 9101, pgid: 9100, role: "helper", state: "orphaned",
          state_label: "Still running after its Instance closed",
          role_guess: "node (guess)", exe_path: "/usr/local/bin/node",
          cmdline: "node vite dev", age_ms: 4_000_000,
          rss_bytes: 220 * 1024 * 1024, cpu_pct: 0.4, ports: "5174",
          hog: false, hog_rules: [],
          evidence: {
            root_pid: 9100, parent_pid: 9100, pgid: 9100, escaped_group: false,
            verification: { verdict: "verified", detail: "match (remote record)" },
            first_seen: T0, last_seen: T0 + 90_000,
          },
        },
      ],
    },
  ],
  local_machine: "machine-local",
  clean_instances: 2,
  ledger_degraded: false,
};

for (const theme of THEMES) {
  for (const vp of [
    { name: "1440x900", width: 1440, height: 900 },
    { name: "1024x700", width: 1024, height: 700 },
  ]) {
    test(`fleet — machine column + filter + foreign display-only — ${vp.name} — ${theme}`, async ({ page }) => {
      await seedRecoveryPage(page, theme, { __AIJ_CLEANUP__: FLEET_CLEANUP });
      await page.getByRole("button", { name: "Cleanup" }).click();
      await expect(page.getByTestId("cleanup-centre")).toBeVisible();
      // The default is This machine — only the local group shows.
      await expect(page.getByTestId("cleanup-machine-8123")).toHaveText("This Mac");
      await expect(page.getByTestId("cleanup-row-9101")).toHaveCount(0);
      // Widen: the foreign row appears, display-only (no kill control).
      await page.getByRole("combobox", { name: "Machine filter" }).selectOption("all");
      await expect(page.getByTestId("cleanup-machine-9101")).toHaveText("machine-studio");
      await expect(page.getByTestId("cleanup-kill-9101")).toHaveCount(0);
      await expect(page.getByTestId("cleanup-kill-8123")).toBeVisible();
      await page.screenshot({ path: resolve(GALLERY, `24-fleet/machine-filter-${vp.name}-${theme}.png`) });
    });
  }
}

/**
 * AJ-E609 — the Usage cost matrix states: priced / unpriced-model /
 * stale-pricing across the viewport × theme matrix. The component
 * reads the __AIJ_COST_ROLLUPS__ seam (no backend), so the states are
 * deterministic fixtures, exactly as the gallery law requires.
 */
const costStates: Record<string, Record<string, unknown>> = {
  priced: {
    rows: [
      {
        key: "sess-alpha", model: "claude-opus-4", day: "2026-10-15",
        tokens: { input: 1_204_318, output: 88_412, cache_read: 902_114, cache_write: 12_004, reasoning: 0 },
        usd: { usd: 24.31, label: "Estimate" }, confidence: "Measured",
      },
      {
        key: "sess-beta", model: "claude-sonnet-4", day: "2026-10-15",
        tokens: { input: 402_118, output: 30_221, cache_read: 200_331, cache_write: 0, reasoning: 0 },
        usd: { usd: 3.92, label: "Estimate" }, confidence: "Measured",
      },
    ],
    subscription: { api_equivalent_usd: 28.23, flat_fee_usd: 200, value_ratio: 0.14 },
    pricing_stale: false, total_usd: 28.23, scope: "ALL",
  },
  "unpriced-model": {
    rows: [
      {
        key: "sess-z", model: "GLM-6-X", day: "2026-10-15",
        tokens: { input: 12_004, output: 1_204, cache_read: 0, cache_write: 0, reasoning: 0 },
        usd: { label: "Estimate" }, credits: 1.25, confidence: "Computed",
      },
    ],
    subscription: {}, pricing_stale: false, scope: "ALL",
  },
  "stale-pricing": {
    rows: [
      {
        key: "sess-alpha", model: "claude-opus-4", day: "2026-09-01",
        tokens: { input: 999_999, output: 50_000, cache_read: 0, cache_write: 0, reasoning: 0 },
        usd: { usd: 15.0, label: "Estimate" }, confidence: "Measured",
      },
    ],
    subscription: {}, pricing_stale: true, total_usd: 15.0,
    pricing_last_synced_ms: T0 - 40 * 24 * 3_600_000, scope: "ALL",
  },
};

for (const theme of THEMES) {
  for (const vp of VIEWPORTS) {
    for (const [state, fixture] of Object.entries(costStates)) {
      test(`cost matrix — ${state} — ${vp.name} — ${theme}`, async ({ page }) => {
        await page.addInitScript(
          ({ rollups, t0, theme: themeName }) => {
            (window as never as { __AIJ_COST_ROLLUPS__: unknown }).__AIJ_COST_ROLLUPS__ = rollups;
            (window as never as { __AIJ_COST_CSV__: unknown }).__AIJ_COST_CSV__ =
              "/tmp/ai-juggler-costs.csv";
            (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = {
              accounts: [], recipes: [], sessions: [], projects: [],
            };
            localStorage.setItem("aij.theme", themeName);
            localStorage.setItem("aij.onboarding.dismissed", "1");
            sessionStorage.setItem("aij.ready.skipped", "1");
            Date.now = () => t0;
          },
          { rollups: fixture, t0: T0, theme },
        );
        await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
        await page.setViewportSize({ width: vp.width, height: vp.height });
        await page.goto("/");
        // The matrix rides under the WorkPanels empty state (no
        // sessions needed — it renders from the seam).
        await expect(
          page.getByText("USD is Estimated — list pricing, not your subscription fee."),
        ).toBeVisible();
        await page.screenshot({
          path: resolve(GALLERY, `25-costs/${state}-${vp.name}-${theme}.png`),
        });
      });
    }
  }
}
