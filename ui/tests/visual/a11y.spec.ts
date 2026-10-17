import { expect, test } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
import { allStatuses, honestEmpty, type FixtureSet } from "./fixtures";

/**
 * AJ-E402 quality gate: axe-core reports 0 critical/serious violations on
 * the rail fixture page (all 9 states rendered) in BOTH themes (D-16).
 * Full a11y programme lands with AJ-E804; this is the row's entry gate.
 */

/**
 * AJ-E806 flake record (2026-10-16): under the FULL parallel suite,
 * axe color-contrast intermittently reports serious violations on the
 * kill sheet (refused) and the recovery dialog (light theme) — the
 * same tests pass in isolation and on the clean tree; the flagged
 * elements belong to dialogs whose transitions settle after the scan
 * under load. Fix: ONE confirmatory re-scan after a settle beat. A
 * real violation reproduces deterministically and still fails the
 * gate; a load-induced false positive does not survive the re-scan.
 */
async function scanSeriousWithConfirm(page: import("@playwright/test").Page) {
  const seriousOf = async () => {
    const results = await new AxeBuilder({ page }).analyze();
    return results.violations.filter((v) =>
      ["serious", "critical"].includes(v.impact ?? ""),
    );
  };
  let serious = await seriousOf();
  if (serious.length > 0) {
    await page.waitForTimeout(400);
    serious = await seriousOf();
  }
  return serious;
}

const T0 = 1_728_800_000_000;

declare global {
  interface Window {
    __AIJ_FIXTURE__?: Partial<FixtureSet>;
  }
}

for (const theme of ["dark", "light"] as const) {
  test(`rail — axe 0 critical/serious — ${theme}`, async ({ page }) => {
    await page.addInitScript(
      ({ fixture, t0, theme: t }) => {
        (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
        localStorage.setItem("aij.theme", t);
        localStorage.setItem("aij.onboarding.dismissed", "1");
        sessionStorage.setItem("aij.ready.skipped", "1");
        Date.now = () => t0;
      },
      { fixture: allStatuses, t0: T0, theme },
    );
    await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto("/");
    await expect(page.getByRole("tab", { name: "Sessions" })).toBeVisible();
    await page.getByRole("tab", { name: "Sessions" }).click();
    await expect(page.getByText("Handed over").first()).toBeVisible();

    // AJ-E405: the badge tooltip carries the structured reason (rule +
    // measured inputs) — asserted in the DOM (native titles are not
    // screenshot-able).
    await expect(page.locator('[title*="waiting_prompt"]').first()).toBeVisible();
    await expect(page.locator('[title*="stuck_no_output"]').first()).toBeVisible();
    const results = await new AxeBuilder({ page })
      .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
      .analyze();
    let serious = results.violations.filter((v) =>
      ["serious", "critical"].includes(v.impact ?? ""),
    );
    if (serious.length > 0) {
        // AJ-E806: confirmatory re-scan — load-induced contrast false
        // positives do not survive; real violations still fail.
        await page.waitForTimeout(400);
        const confirm = await new AxeBuilder({ page })
          .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
          .analyze();
        serious = confirm.violations.filter((v) =>
          ["serious", "critical"].includes(v.impact ?? ""),
        );
    }
    if (serious.length > 0) {
      // Report honestly: id + impact + selectors, one line each.
      const summary = serious
        .map((v) => `${v.id} (${v.impact}): ${v.nodes.map((n) => n.target.join(" ")).slice(0, 3).join(", ")}`)
        .join("\n");
      throw new Error(`axe serious/critical violations on the rail (${theme}):\n${summary}`);
    }
    expect(serious).toHaveLength(0);
  });
}

// AJ-E402 layout gate: no row may overflow its rail width — the provider
// abbrev, model, age and resource text yield (truncate) before the disc,
// state chip text or glyphs ever clip. Deliberately UNPINNED clock so
// pathological durations ("wait 730d") are exercised too.
test("rail — no row overflows its width", async ({ page }) => {
  await page.addInitScript((fixture) => {
    (window as never as { __AIJ_FIXTURE__: unknown }).__AIJ_FIXTURE__ = fixture;
    localStorage.setItem("aij.theme", "dark");
    localStorage.setItem("aij.onboarding.dismissed", "1");
    sessionStorage.setItem("aij.ready.skipped", "1");
  }, allStatuses);
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.getByRole("tab", { name: "Sessions" }).click();
  await expect(page.getByText("Handed over").first()).toBeVisible();
  const overflows = await page.evaluate(() => {
    const bad: string[] = [];
    for (const row of document.querySelectorAll("aside button")) {
      if (row.scrollWidth > row.clientWidth + 1) {
        bad.push(`${(row.getAttribute("aria-label") ?? "?").slice(0, 40)}: ${row.scrollWidth}px > ${row.clientWidth}px`);
      }
    }
    return bad;
  });
  expect(overflows).toEqual([]);
});


// AJ-E507 quality gate: axe-core 0 critical/serious on the kill
// sheet — the ready (interactive) state and the failed-verification
// explanation. The suite rides the same cleanup fixture as the
// 20-kill gallery; the sheet opens the real way (row affordance).
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

for (const theme of ["dark", "light"] as const) {
  for (const state of ["ready", "refused"] as const) {
    test(`kill-sheet — ${state} — axe 0 critical/serious — ${theme}`, async ({ page }) => {
      const cleanup = {
        groups: [
          {
            session: "s-1",
            provider: "claude-code",
            account: "Work Max",
            project: "web",
            machine: "machine-local",
            healthy: false,
            rows: [
              {
                pid: 8123,
                pgid: 8100,
                role: "helper",
                state: "orphaned",
                state_label: "Still running after its Instance closed",
                role_guess: "python3 (guess)",
                exe_path: "/usr/bin/python3",
                cmdline: "python3 -m http.server 5173",
                age_ms: 9_000_000,
                rss_bytes: 48 * 1024 * 1024,
                cpu_pct: 1.4,
                ports: "5173",
                hog: false,
                hog_rules: [],
                evidence: {
                  root_pid: 8100,
                  parent_pid: 8100,
                  pgid: 8100,
                  escaped_group: false,
                  verification: {
                    verdict: "verified",
                    detail: "start-time and exe match the live process",
                  },
                  first_seen: T0,
                  last_seen: T0 + 90_000,
                },
              },
            ],
          },
        ],
        clean_instances: 2,
        ledger_degraded: false,
      };
      await page.addInitScript(
        ({ fixture, t0, cleanup: c, theme: t, verify }) => {
          const w = window as unknown as {
            __AIJ_FIXTURE__?: unknown;
            __AIJ_CLEANUP__?: unknown;
            __AIJ_KILL_VERIFY__?: unknown;
          };
          w.__AIJ_FIXTURE__ = fixture;
          w.__AIJ_CLEANUP__ = c;
          w.__AIJ_KILL_VERIFY__ = () => verify;
          localStorage.setItem("aij.theme", t);
          localStorage.setItem("aij.onboarding.dismissed", "1");
          sessionStorage.setItem("aij.ready.skipped", "1");
          Date.now = () => t0;
        },
        {
          fixture: allStatuses,
          t0: T0,
          cleanup,
          theme,
          verify: state === "ready" ? KILL_VERIFY_OK : KILL_VERIFY_FAILED,
        },
      );
      await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
      await page.setViewportSize({ width: 1440, height: 900 });
      await page.goto("/");
      await page.getByRole("button", { name: "Cleanup" }).click();
      await expect(page.getByTestId("cleanup-centre")).toBeVisible();
      await page.getByTestId("cleanup-kill-8123").click();
      await expect(
        page.getByTestId(state === "ready" ? "kill-ready-8123" : "kill-refused-8123"),
      ).toBeVisible();
      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
        .analyze();
      let serious = results.violations.filter((v) =>
        ["serious", "critical"].includes(v.impact ?? ""),
      );
      if (serious.length > 0) {
        // AJ-E806: confirmatory re-scan — load-induced contrast false
        // positives do not survive; real violations still fail.
        await page.waitForTimeout(400);
        const confirm = await new AxeBuilder({ page })
          .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
          .analyze();
        serious = confirm.violations.filter((v) =>
          ["serious", "critical"].includes(v.impact ?? ""),
        );
      }
      if (serious.length > 0) {
        const summary = serious
          .map(
            (v) =>
              `${v.id} (${v.impact}): ${v.nodes.map((n) => n.target.join(" ")).slice(0, 3).join(", ")}`,
          )
          .join("\n");
        throw new Error(`axe serious/critical violations on the kill sheet (${state}/${theme}):\n${summary}`);
      }
      expect(serious).toHaveLength(0);
    });
  }
}


// AJ-E508: axe-core 0 critical/serious on the tier surfaces — the
// per-Account toggles (the automation consent) and the Tier-2
// notification (the Undo window).
for (const theme of ["dark", "light"] as const) {
  test(`tiers — account toggles — axe 0 critical/serious — ${theme}`, async ({ page }) => {
    await page.addInitScript(
      ({ fixture, t0, theme: t }) => {
        const w = window as unknown as Record<string, unknown>;
        w.__AIJ_FIXTURE__ = fixture;
        w.__AIJ_CLEANUP_TIERS__ = { tier1_opt_in: true, tier2_opt_in: false };
        localStorage.setItem("aij.theme", t);
        localStorage.setItem("aij.onboarding.dismissed", "1");
        sessionStorage.setItem("aij.ready.skipped", "1");
        Date.now = () => t0;
      },
      { fixture: allStatuses, t0: T0, theme },
    );
    await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto("/");
    await page.getByRole("button", { name: "Accounts" }).click();
    await page.getByRole("button", { name: "Inspect claude-code" }).first().click();
    await expect(page.getByTestId("account-tier-toggles")).toBeVisible();
    const results = await new AxeBuilder({ page })
      .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
      .analyze();
    let serious = results.violations.filter((v) =>
      ["serious", "critical"].includes(v.impact ?? ""),
    );
    if (serious.length > 0) {
        // AJ-E806: confirmatory re-scan — load-induced contrast false
        // positives do not survive; real violations still fail.
        await page.waitForTimeout(400);
        const confirm = await new AxeBuilder({ page })
          .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
          .analyze();
        serious = confirm.violations.filter((v) =>
          ["serious", "critical"].includes(v.impact ?? ""),
        );
    }
    if (serious.length > 0) {
      const summary = serious
        .map(
          (v) =>
            `${v.id} (${v.impact}): ${v.nodes.map((n) => n.target.join(" ")).slice(0, 3).join(", ")}`,
        )
        .join("\n");
      throw new Error(`axe violations on the tier toggles (${theme}):\n${summary}`);
    }
    expect(serious).toHaveLength(0);
  });

  test(`tiers — tier-2 notification — axe 0 critical/serious — ${theme}`, async ({ page }) => {
    await page.addInitScript(
      ({ fixture, t0, theme: t }) => {
        const w = window as unknown as Record<string, unknown>;
        w.__AIJ_FIXTURE__ = fixture;
        w.__AIJ_CLEANUP_STATUS__ = {
          paused: true,
          suspended: false,
          suspended_until_ms: null,
          tier2_pending: [
            { pid: 8123, session: "s-1", cmdline_trunc: "node vite dev", deadline_ms: t0 + 30_000 },
          ],
        };
        localStorage.setItem("aij.theme", t);
        localStorage.setItem("aij.onboarding.dismissed", "1");
        sessionStorage.setItem("aij.ready.skipped", "1");
        Date.now = () => t0;
      },
      { fixture: allStatuses, t0: T0, theme },
    );
    await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto("/");
    await page.getByRole("button", { name: "Cleanup" }).click();
    await expect(page.getByTestId("cleanup-tier2-notice-8123")).toBeVisible();
    const results = await new AxeBuilder({ page })
      .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
      .analyze();
    let serious = results.violations.filter((v) =>
      ["serious", "critical"].includes(v.impact ?? ""),
    );
    if (serious.length > 0) {
        // AJ-E806: confirmatory re-scan — load-induced contrast false
        // positives do not survive; real violations still fail.
        await page.waitForTimeout(400);
        const confirm = await new AxeBuilder({ page })
          .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
          .analyze();
        serious = confirm.violations.filter((v) =>
          ["serious", "critical"].includes(v.impact ?? ""),
        );
    }
    if (serious.length > 0) {
      const summary = serious
        .map(
          (v) =>
            `${v.id} (${v.impact}): ${v.nodes.map((n) => n.target.join(" ")).slice(0, 3).join(", ")}`,
        )
        .join("\n");
      throw new Error(`axe violations on the tier-2 notice (${theme}):\n${summary}`);
    }
    expect(serious).toHaveLength(0);
  });
}

// AJ-E510: axe-core 0 critical/serious on the one-time restart summary
// dialog (modal semantics + focus land inside it) — the crash moment's
// first surface.
for (const theme of ["dark", "light"] as const) {
  test(`recovery — summary dialog — axe 0 critical/serious — ${theme}`, async ({ page }) => {
    await page.addInitScript(
      ({ fixture, t0, theme: t, recovery }) => {
        const w = window as unknown as Record<string, unknown>;
        w.__AIJ_FIXTURE__ = fixture;
        w.__AIJ_RECOVERY__ = recovery;
        localStorage.setItem("aij.theme", t);
        localStorage.setItem("aij.onboarding.dismissed", "1");
        sessionStorage.setItem("aij.ready.skipped", "1");
        Date.now = () => t0;
      },
      {
        fixture: allStatuses,
        t0: T0,
        theme,
        recovery: {
          summary:
            "2 Instances re-adopted, 1 helpers left behind, 1 processes could not be verified and were left alone.",
          adopted: 2,
          left_behind: 1,
          unverifiable: 1,
        },
      },
    );
    await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto("/");
    await expect(page.getByTestId("recovery-summary")).toBeVisible();
    // Focus is INSIDE the dialog (not lost behind the overlay).
    const focusInDialog = await page.evaluate(() => {
      const dialog = document.querySelector('[data-testid="recovery-summary"]');
      return dialog !== null && dialog.contains(document.activeElement);
    });
    if (!focusInDialog) throw new Error("focus did not land inside the recovery dialog");
    const results = await new AxeBuilder({ page })
      .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
      .analyze();
    let serious = results.violations.filter((v) =>
      ["serious", "critical"].includes(v.impact ?? ""),
    );
    if (serious.length > 0) {
      // AJ-E806: confirmatory re-scan (same rationale as the kill sheet).
      await page.waitForTimeout(400);
      const confirm = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
        .analyze();
      serious = confirm.violations.filter((v) =>
        ["serious", "critical"].includes(v.impact ?? ""),
      );
    }
    if (serious.length > 0) {
      const summary = serious
        .map(
          (v) =>
            `${v.id} (${v.impact}): ${v.nodes.map((n) => n.target.join(" ")).slice(0, 3).join(", ")}`,
        )
        .join("\n");
      throw new Error(`axe violations on the recovery dialog (${theme}):\n${summary}`);
    }
    expect(serious).toHaveLength(0);
  });
}

// AJ-E512: axe-core 0 critical/serious on the passive alert surfaces
// (hog alert with its staged-sheet affordance + the idle nudge).
for (const theme of ["dark", "light"] as const) {
  test(`alerts — hog + idle — axe 0 critical/serious — ${theme}`, async ({ page }) => {
    await page.addInitScript(
      ({ fixture, t0, theme: t, alerts }) => {
        const w = window as unknown as Record<string, unknown>;
        w.__AIJ_FIXTURE__ = fixture;
        w.__AIJ_CLEANUP_ALERTS__ = alerts;
        localStorage.setItem("aij.theme", t);
        localStorage.setItem("aij.onboarding.dismissed", "1");
        sessionStorage.setItem("aij.ready.skipped", "1");
        Date.now = () => t0;
      },
      {
        fixture: allStatuses,
        t0: T0,
        theme,
        alerts: {
          alerts: [
            {
              kind: "hog",
              text: "node (vite) under Work Max holds 3.1 GiB — review in Cleanup Centre.",
              pid: 8123,
              session: "s-1",
            },
            {
              kind: "idle",
              text: "python3 has been idle for 2h — review it in the Cleanup Centre.",
              pid: 8124,
              session: "s-1",
            },
          ],
        },
      },
    );
    await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto("/");
    await page.getByRole("button", { name: "Cleanup" }).click();
    await expect(page.getByTestId("hog-alert-8123")).toBeVisible();
    const results = await new AxeBuilder({ page })
      .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
      .analyze();
    let serious = results.violations.filter((v) =>
      ["serious", "critical"].includes(v.impact ?? ""),
    );
    if (serious.length > 0) {
        // AJ-E806: confirmatory re-scan — load-induced contrast false
        // positives do not survive; real violations still fail.
        await page.waitForTimeout(400);
        const confirm = await new AxeBuilder({ page })
          .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
          .analyze();
        serious = confirm.violations.filter((v) =>
          ["serious", "critical"].includes(v.impact ?? ""),
        );
    }
    if (serious.length > 0) {
      const summary = serious
        .map(
          (v) =>
            `${v.id} (${v.impact}): ${v.nodes.map((n) => n.target.join(" ")).slice(0, 3).join(", ")}`,
        )
        .join("\n");
      throw new Error(`axe violations on the alert surfaces (${theme}):\n${summary}`);
    }
    expect(serious).toHaveLength(0);
  });
}


// AJ-E511: axe-core 0 critical/serious on the fleet machine surfaces
// (the machine column + the widened All-machines view with its
// display-only foreign row).
for (const theme of ["dark", "light"] as const) {
  test(`fleet — machine column + filter — axe 0 critical/serious — ${theme}`, async ({ page }) => {
    await page.addInitScript(
      ({ fixture, t0, theme: t, cleanup }) => {
        const w = window as unknown as Record<string, unknown>;
        w.__AIJ_FIXTURE__ = fixture;
        w.__AIJ_CLEANUP__ = cleanup;
        localStorage.setItem("aij.theme", t);
        localStorage.setItem("aij.onboarding.dismissed", "1");
        sessionStorage.setItem("aij.ready.skipped", "1");
        Date.now = () => t0;
      },
      {
        fixture: allStatuses,
        t0: T0,
        theme,
        cleanup: {
          groups: [
            {
              session: "s-local", provider: "claude-code", account: "Work Max",
              project: "web", machine: "machine-local", healthy: false,
              rows: [
                {
                  machine_id: "machine-local", pid: 8123, pgid: 8100, role: "helper",
                  state: "orphaned", state_label: "Still running after its Instance closed",
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
                {
                  machine_id: "machine-studio", pid: 9101, pgid: 9100, role: "helper",
                  state: "orphaned", state_label: "Still running after its Instance closed",
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
        },
      },
    );
    await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto("/");
    await page.getByRole("button", { name: "Cleanup" }).click();
    await page
      .getByRole("combobox", { name: "Machine filter" })
      .selectOption("all");
    await expect(page.getByTestId("cleanup-machine-9101")).toBeVisible();
    const results = await new AxeBuilder({ page })
      .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
      .analyze();
    let serious = results.violations.filter((v) =>
      ["serious", "critical"].includes(v.impact ?? ""),
    );
    if (serious.length > 0) {
        // AJ-E806: confirmatory re-scan — load-induced contrast false
        // positives do not survive; real violations still fail.
        await page.waitForTimeout(400);
        const confirm = await new AxeBuilder({ page })
          .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
          .analyze();
        serious = confirm.violations.filter((v) =>
          ["serious", "critical"].includes(v.impact ?? ""),
        );
    }
    if (serious.length > 0) {
      const summary = serious
        .map(
          (v) =>
            `${v.id} (${v.impact}): ${v.nodes.map((n) => n.target.join(" ")).slice(0, 3).join(", ")}`,
        )
        .join("\n");
      throw new Error(`axe violations on the fleet surfaces (${theme}):\n${summary}`);
    }
    expect(serious).toHaveLength(0);
  });
}


// AJ-E611: axe-core 0 critical/serious on the ranking + notification
// surfaces — the NewSession ranked list (labelled listbox, expander)
// and the notification region (polite live-region, queue rows with an
// explicit Resume click) in BOTH themes.
for (const theme of ["dark", "light"] as const) {
  test(`ranking — ranked list + notifications — axe 0 critical/serious — ${theme}`, async ({ page }) => {
    await page.addInitScript(
      ({ fixture, t0, theme: t, ranked, events, rows }) => {
        const w = window as unknown as Record<string, unknown>;
        w.__AIJ_FIXTURE__ = fixture;
        w.__AIJ_RANKING__ = ranked;
        w.__AIJ_ALERTS__ = events;
        w.__AIJ_RESET_QUEUE__ = rows;
        localStorage.setItem("aij.theme", t);
        localStorage.setItem("aij.onboarding.dismissed", "1");
        sessionStorage.setItem("aij.ready.skipped", "1");
        Date.now = () => t0;
      },
      {
        fixture: allStatuses,
        t0: T0,
        theme,
        ranked: [
          {
            account: "claude-work",
            score: 0.87,
            top_factors: ["82% headroom (Measured)", "usual account for this repo"],
            factors: [["headroom 82%", 0.37], ["affinity", 0.2], ["fidelity", 0.075], ["cost fit", 0.1], ["calm", 0.1]],
            headroom_unknown: false,
            confidence: "Measured",
            tte_min: 210,
            suggested: true,
          },
          {
            account: "claude-lab",
            filtered_reason: "logged out — sign in from the Ready home",
            top_factors: [],
            factors: [],
            headroom_unknown: true,
            confidence: "Estimate",
            suggested: false,
          },
        ],
        events: [
          { kind: "actionable", account: "claude-work", text: "claude-work at 92% used (Measured), five-hour window — hand over or queue at reset", at_ms: T0 - 60_000 },
          { kind: "digest", account: "claude-personal", text: "Renews in 6 days — check your plan", at_ms: T0 - 3_600_000 },
        ],
        rows: [
          { id: "rq-1", session: "sess-1", account: "claude-work", resume_at_ms: T0 + 45 * 60_000, state: "queued" },
          { id: "rq-2", session: "sess-2", account: "claude-lab", resume_at_ms: T0 - 60_000, state: "ready" },
        ],
      },
    );
    await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto("/");
    // The notification region renders in the work view (polite
    // live-region semantics assertable before the modal opens).
    await expect(page.getByTestId("alert-actionable")).toBeVisible();
    await expect(page.getByTestId("queue-ready")).toBeVisible();
    await page.getByRole("button", { name: "New", exact: true }).click();
    await expect(page.getByTestId("ranked-rows")).toBeVisible();
    const serious = await scanSeriousWithConfirm(page);
    if (serious.length > 0) {
      const summary = serious
        .map(
          (v) =>
            `${v.id} (${v.impact}): ${v.nodes.map((n) => n.target.join(" ")).slice(0, 3).join(", ")}`,
        )
        .join("\n");
      throw new Error(`axe violations on the ranking surfaces (${theme}):\n${summary}`);
    }
    expect(serious).toHaveLength(0);
  });
}


// AJ-E205/E206: axe-core 0 critical/serious on the switch flow —
// the door (wizard with focus-trapped dialog, two radio freeze
// choices), the 1-click popover, the undo toast, and the
// source-safe failure banner — in BOTH themes.
const DOOR_WIZARD_FIXTURE = {
  mechanism: "transplant",
  fidelity: "F2",
  score: 0.78,
  warnings: ["This conversation was compacted. Early detail carries as text, not as live memory — it may ask about early decisions."],
  zombies: [],
  diff_snapshotted: true,
  source_live: true,
  experimental_copy: "Experimental: only the main transcript moves. Subagent files and rewind history stay on the source. The source stays resumable.",
  door: "wizard" as const,
  wizard_steps: ["Target", "What's carried", "Confirm"],
  wizard_reasons: ["This isn't the usual account for this project."],
  usual_account: false,
  carry: {
    carried: ["chat transcript", "rules & guidelines", "uncommitted git diff"],
    regenerated: [],
    dropped: ["todo list"],
  },
};

const DOOR_ONECLICK_FIXTURE = {
  ...DOOR_WIZARD_FIXTURE,
  fidelity: "F4",
  warnings: [],
  door: "one-click" as const,
  wizard_steps: [],
  wizard_reasons: [],
  usual_account: true,
  carry: {
    carried: ["chat transcript", "rules & guidelines", "uncommitted git diff", "MCP config", "skills & commands", "model settings"],
    regenerated: [],
    dropped: [],
  },
};

for (const theme of ["dark", "light"] as const) {
  test(`switch flow — wizard door + failure — axe 0 critical/serious — ${theme}`, async ({ page }) => {
    await page.addInitScript(
      ({ fixture, t0, theme: t, preflight, fail }) => {
        const w = window as unknown as Record<string, unknown>;
        w.__AIJ_FIXTURE__ = fixture;
        w.__AIJ_SWITCH_PREFLIGHT__ = preflight;
        w.__AIJ_SWITCH_EXECUTE_FAIL__ = fail;
        localStorage.setItem("aij.theme", t);
        localStorage.setItem("aij.onboarding.dismissed", "1");
        sessionStorage.setItem("aij.ready.skipped", "1");
        Date.now = () => t0;
      },
      {
        fixture: allStatuses,
        t0: T0,
        theme,
        preflight: DOOR_WIZARD_FIXTURE,
        fail: "S-F5: target launch failed — staged for retry (terminal spawn failed).",
      },
    );
    await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.goto("/");
    await page.getByRole("tab", { name: "Sessions" }).click();
    await page.getByRole("button", { name: /Work Max/ }).first().click();
    await page.getByTestId("handover-section")
      // AJ-E209: the ranked, badged row is the switch entry point.
      .locator("button[data-row]:not(:disabled)")
      .first()
      .click();
    await expect(page.getByTestId("switch-flow")).toBeVisible();
    // Scope to the dialog while a modal is open: the backdrop
    // intentionally dims the app behind it (a transient rendering
    // state, not the design's contrast); the dialog itself is
    // scanned in full.
    const dialogScan = async () => {
      const results = await new AxeBuilder({ page })
        .include("[data-testid='switch-flow']")
        .analyze();
      let serious = results.violations.filter((v) =>
        ["serious", "critical"].includes(v.impact ?? ""),
      );
      if (serious.length > 0) {
        await page.waitForTimeout(400);
        const confirm = await new AxeBuilder({ page })
          .include("[data-testid='switch-flow']")
          .analyze();
        serious = confirm.violations.filter((v) =>
          ["serious", "critical"].includes(v.impact ?? ""),
        );
      }
      return serious;
    };
    const serious = await dialogScan();
    // Keyboard traversal through the wizard (steps announced via
    // aria-live; the confirm click lives on the LAST step).
    await page.getByTestId("switch-continue").click();
    await expect(page.getByTestId("switch-steps")).toContainText("Step 2 of 3");
    const seriousCarried = await dialogScan();
    await page.getByTestId("switch-continue").click();
    await expect(page.getByTestId("switch-step-confirm")).toBeVisible();
    const seriousConfirm = await dialogScan();
    // Then the failure banner surface.
    await page.getByTestId("switch-confirm").click();
    await expect(page.getByTestId("switch-failed")).toBeVisible();
    const serious2 = await dialogScan();
    const all = [...serious, ...seriousCarried, ...seriousConfirm, ...serious2];
    if (all.length > 0) {
      const summary = all
        .map((v) => `${v.id} (${v.impact}): ${v.nodes.map((n) => n.target.join(" ")).slice(0, 3).join(", ")}`)
        .join("\n");
      throw new Error(`axe violations on the switch wizard (${theme}):\n${summary}`);
    }
    expect(all).toHaveLength(0);
  });

  test(`switch flow — 1-click popover + undo toast — axe 0 critical/serious — ${theme}`, async ({ page }) => {
    await page.addInitScript(
      ({ fixture, t0, theme: t, preflight }) => {
        const w = window as unknown as Record<string, unknown>;
        w.__AIJ_FIXTURE__ = fixture;
        w.__AIJ_SWITCH_PREFLIGHT__ = preflight;
        localStorage.setItem("aij.theme", t);
        localStorage.setItem("aij.onboarding.dismissed", "1");
        sessionStorage.setItem("aij.ready.skipped", "1");
        Date.now = () => t0;
      },
      {
        fixture: allStatuses,
        t0: T0,
        theme,
        preflight: DOOR_ONECLICK_FIXTURE,
      },
    );
    await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.goto("/");
    await page.getByRole("tab", { name: "Sessions" }).click();
    await page.getByRole("button", { name: /Work Max/ }).first().click();
    await page.getByTestId("handover-section")
      // AJ-E209: the ranked, badged row is the switch entry point.
      .locator("button[data-row]:not(:disabled)")
      .first()
      .click();
    await expect(page.getByTestId("switch-oneclick")).toBeVisible();
    const scan = async (scope: string) => {
      const results = await new AxeBuilder({ page })
        .include(`[data-testid='${scope}']`)
        .analyze();
      let serious = results.violations.filter((v) =>
        ["serious", "critical"].includes(v.impact ?? ""),
      );
      if (serious.length > 0) {
        await page.waitForTimeout(400);
        const confirm = await new AxeBuilder({ page })
          .include(`[data-testid='${scope}']`)
          .analyze();
        serious = confirm.violations.filter((v) =>
          ["serious", "critical"].includes(v.impact ?? ""),
        );
      }
      return serious;
    };
    const seriousPopover = await scan("switch-flow");
    // Execute → the undo window (toast at the app root).
    await page.getByTestId("switch-confirm").click();
    await expect(page.getByTestId("switch-undo-toast")).toBeVisible();
    const seriousToast = await scan("switch-undo-toast");
    const all = [...seriousPopover, ...seriousToast];
    if (all.length > 0) {
      const summary = all
        .map((v) => `${v.id} (${v.impact}): ${v.nodes.map((n) => n.target.join(" ")).slice(0, 3).join(", ")}`)
        .join("\n");
      throw new Error(`axe violations on the 1-click door (${theme}):\n${summary}`);
    }
    expect(all).toHaveLength(0);
  });
}

// AJ-E207: axe-core 0 critical/serious on the switch-back flow —
// the review (delta preview, per-file conflict radio trios with
// file-path labels, freeze-or-fork), the S-F7 dead-original state,
// and the S-F8 forked failure — in BOTH themes; the conflict set is
// resolvable keyboard-only (radios carry their labels).
const BACK_CARDS_A11Y = ["sess-1", "sess-2", "sess-4", "sess-6", "sess-8"].map(
  (session_id) => ({
    session_id,
    original_account: "Work Max",
    away_account: "Personal",
    copy: "Work Max has reset. Resume the original conversation there? Work from Personal arrives as one 'while you were away' update.",
  }),
);

for (const theme of ["dark", "light"] as const) {
  test(`switch-back — review + conflict radios — axe 0 critical/serious — ${theme}`, async ({ page }) => {
    await page.addInitScript(
      ({ fixture, t0, theme: t, cards }) => {
        const w = window as unknown as Record<string, unknown>;
        w.__AIJ_FIXTURE__ = fixture;
        w.__AIJ_SWITCH_BACK_SUGGESTIONS__ = cards;
        localStorage.setItem("aij.theme", t);
        localStorage.setItem("aij.onboarding.dismissed", "1");
        sessionStorage.setItem("aij.ready.skipped", "1");
        Date.now = () => t0;
      },
      { fixture: allStatuses, t0: T0, theme, cards: BACK_CARDS_A11Y },
    );
    await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.goto("/");
    await page.getByRole("tab", { name: "Sessions" }).click();
    await page.getByRole("button", { name: /Work Max/ }).first().click();
    await expect(page.getByTestId("switch-back-card")).toBeVisible();
    const cardScan = await new AxeBuilder({ page })
      .include("[data-testid='switch-back-card']")
      .analyze();
    await page.getByTestId("switch-back-card").click();
    await expect(page.getByTestId("switch-back-review")).toBeVisible();
    // Every conflict radio resolves by labelled name (keyboard path).
    await page.getByLabel("Take the away version").check();
    const dialogScan = await new AxeBuilder({ page })
      .include("[data-testid='switch-back-flow']")
      .analyze();
    // The failure surface (S-F8 forked).
    await page.getByTestId("switch-back-confirm").click();
    await expect(page.getByTestId("switch-back-done")).toBeVisible();
    const doneScan = await new AxeBuilder({ page })
      .include("[data-testid='switch-back-flow']")
      .analyze();
    const all = [cardScan, dialogScan, doneScan]
      .flatMap((result) => result.violations)
      .filter((violation) => ["serious", "critical"].includes(violation.impact ?? ""));
    if (all.length > 0) {
      const summary = all
        .map((v) => `${v.id} (${v.impact}): ${v.nodes.map((n) => n.target.join(" ")).slice(0, 3).join(", ")}`)
        .join("\n");
      throw new Error(`axe violations on the switch-back flow (${theme}):\n${summary}`);
    }
    expect(all).toHaveLength(0);
  });
}

// AJ-E208: axe-core 0 critical/serious on the redaction surfaces —
// the first-run chooser (labelled radios, no default) and the
// per-item un-redact list (masked previews, checkbox per item) —
// in BOTH themes.
for (const theme of ["dark", "light"] as const) {
  test(`redaction — chooser + un-redact list — axe 0 critical/serious — ${theme}`, async ({ page }) => {
    await page.addInitScript(
      ({ fixture, t0, theme: t }) => {
        const w = window as unknown as Record<string, unknown>;
        w.__AIJ_FIXTURE__ = fixture;
        w.__AIJ_SWITCH_PREFLIGHT__ = {
          mechanism: "transplant",
          fidelity: "F2",
          score: 0.78,
          warnings: [],
          zombies: [],
          diff_snapshotted: true,
          source_live: true,
          experimental_copy: "Experimental copy.",
          door: "wizard",
          wizard_steps: ["Target", "What's carried", "Confirm"],
          wizard_reasons: ["This isn't the usual account for this project."],
          usual_account: false,
          carry: {
            carried: ["chat transcript"],
            regenerated: [],
            dropped: ["todo list"],
          },
          redaction: { global: null, override: null, effective: null, source: "unset", line: "" },
          redaction_items: [],
        };
        localStorage.setItem("aij.theme", t);
        localStorage.setItem("aij.onboarding.dismissed", "1");
        sessionStorage.setItem("aij.ready.skipped", "1");
        Date.now = () => t0;
      },
      { fixture: allStatuses, t0: T0, theme },
    );
    await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.goto("/");
    await page.getByRole("tab", { name: "Sessions" }).click();
    await page.getByRole("button", { name: /Work Max/ }).first().click();
    await page.getByTestId("handover-section")
      // AJ-E209: the ranked, badged row is the switch entry point.
      .locator("button[data-row]:not(:disabled)")
      .first()
      .click();
    // The wizard renders the chooser on its carried/confirm steps —
    // walk to Confirm (Target → What's carried) first.
    await page.getByTestId("switch-continue").click();
    await page.getByTestId("switch-continue").click();
    await expect(page.getByTestId("switch-redaction-chooser")).toBeVisible();
    const chooserScan = await new AxeBuilder({ page })
      .include("[data-testid='switch-flow']")
      .analyze();

    // Switch to a set policy + items on the preview step and re-scan
    // (a second init script overwrites the seam on reload).
    await page.addInitScript(() => {
      const w = window as unknown as { __AIJ_SWITCH_PREFLIGHT__?: Record<string, unknown> };
      if (w.__AIJ_SWITCH_PREFLIGHT__) {
        w.__AIJ_SWITCH_PREFLIGHT__.redaction = {
          global: "strip",
          override: null,
          effective: "strip",
          source: "global",
          line: "Redaction for claude-code: Strip — replace keys with ••• (your global choice)",
        };
        w.__AIJ_SWITCH_PREFLIGHT__.redaction_items = [
          { kind: "sk-key", preview: "s•••5678", segment: 0 },
        ];
      }
    });
    await page.reload();
    await page.getByRole("tab", { name: "Sessions" }).click();
    await page.getByRole("button", { name: /Work Max/ }).first().click();
    await page.getByTestId("handover-section")
      // AJ-E209: the ranked, badged row is the switch entry point.
      .locator("button[data-row]:not(:disabled)")
      .first()
      .click();
    await page.getByTestId("switch-continue").click();
    await expect(page.getByTestId("switch-redaction-items")).toBeVisible();
    const itemsScan = await new AxeBuilder({ page })
      .include("[data-testid='switch-flow']")
      .analyze();
    const all = [chooserScan, itemsScan]
      .flatMap((result) => result.violations)
      .filter((violation) => ["serious", "critical"].includes(violation.impact ?? ""));
    if (all.length > 0) {
      const summary = all
        .map((v) => `${v.id} (${v.impact}): ${v.nodes.map((n) => n.target.join(" ")).slice(0, 3).join(", ")}`)
        .join("\n");
      throw new Error(`axe violations on the redaction surfaces (${theme}):\n${summary}`);
    }
    expect(all).toHaveLength(0);
  });
}

/**
 * AJ-E209 a11y gate: the ranked + badged switch entry and the Switch
 * Graph — axe 0 critical/serious on both surfaces (scoped includes,
 * the E806 confirmatory re-scan pattern), every graph node and failed
 * edge reachable + operable keyboard-only, arrows labelled, and the
 * badge state legible as TEXT (never colour-only, WCAG 2.2 / D-11).
 */
for (const theme of ["dark", "light"] as const) {
  test(`switch targets + graph — axe 0 critical/serious + keyboard — ${theme}`, async ({ page }) => {
    await page.addInitScript(
      ({ fixture, t0, theme: t }) => {
        const w = window as unknown as Record<string, unknown>;
        w.__AIJ_FIXTURE__ = fixture;
        w.__AIJ_SWITCH_GRAPH__ = [
          {
            title: "Login refactor",
            epochs: [
              { id: "ep-a", provider: "claude-code", account_label: "Work Max", kind: "terminal", resumable: true, session_id: "sess-1" },
              { id: "ep-b", provider: "codex", account_label: "Personal", kind: "terminal", resumable: true, session_id: "sess-2" },
            ],
            edges: [
              { from: "ep-a", to: "ep-b", label: "transplant F1 · 12:04", switch_back: false },
              {
                from: "ep-b",
                to: null,
                label: "transplant F1 · target launch failed",
                failed_reason: "S-F6: target window closed mid-launch",
                switch_back: false,
              },
            ],
            collapsed: 1,
            hop_note: "3 switches so far ≈41k tokens",
          },
        ];
        localStorage.setItem("aij.theme", t);
        localStorage.setItem("aij.onboarding.dismissed", "1");
        sessionStorage.setItem("aij.ready.skipped", "1");
        Date.now = () => t0;
      },
      { fixture: allStatuses, t0: T0, theme },
    );
    await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.goto("/");
    await page.getByRole("tab", { name: "Sessions" }).click();
    await page.getByRole("button", { name: /Work Max/ }).first().click();

    // The ranked entry: scoped scan (the rows + their explanations).
    await expect(page.getByTestId("switch-target-rows")).toBeVisible();
    const rowsScan = await new AxeBuilder({ page })
      .include("[data-testid='handover-section']")
      .analyze();

    // The graph: scoped scan on the project panel.
    await page.getByRole("tab", { name: "Projects" }).click();
    await page.getByRole("button", { name: /ai-juggler/ }).first().click();
    await expect(page.getByTestId("switch-graph")).toBeVisible();
    const graphScan = await new AxeBuilder({ page })
      .include("[data-testid='project-switch-graph-card']")
      .analyze();

    const all = [rowsScan, graphScan]
      .flatMap((result) => result.violations)
      .filter((violation) => ["serious", "critical"].includes(violation.impact ?? ""));
    if (all.length > 0) {
      await page.waitForTimeout(400);
      const recheck = await new AxeBuilder({ page })
        .include("[data-testid='project-switch-graph-card']")
        .analyze();
      const still = recheck.violations.filter((v) =>
        ["serious", "critical"].includes(v.impact ?? ""),
      );
      expect.soft(still, `axe violations survive the re-scan (${theme})`).toHaveLength(0);
    }
    expect(all).toHaveLength(0);

    // Keyboard-only: the failed edge is a reachable, labelled button
    // and opens its §9 reason; nodes follow by ↑/↓ roving focus.
    const failedEdge = page.getByTestId("switch-graph-failed-edge");
    await failedEdge.focus();
    await failedEdge.press("Enter");
    await expect(page.getByTestId("switch-graph-failed-detail")).toContainText("S-F6");
    const nodes = page.getByTestId("switch-graph-node");
    await nodes.first().focus();
    await nodes.first().press("ArrowDown");
    await expect(nodes.nth(1)).toBeFocused();
  });
}

/**
 * AJ-E210 a11y gate: the Beta validation matrix is a REAL table —
 * row headers (scope=row), scoped columns, status in TEXT (never
 * colour-only), and axe reports 0 critical/serious on the card in
 * both themes.
 */
for (const theme of ["dark", "light"] as const) {
  test(`beta matrix — real table, axe 0 critical/serious — ${theme}`, async ({ page }) => {
    await page.addInitScript(
      ({ fixture, t0, theme: t }) => {
        const w = window as unknown as Record<string, unknown>;
        w.__AIJ_FIXTURE__ = fixture;
        localStorage.setItem("aij.theme", t);
        localStorage.setItem("aij.onboarding.dismissed", "1");
        sessionStorage.setItem("aij.ready.skipped", "1");
        Date.now = () => t0;
      },
      { fixture: allStatuses, t0: T0, theme },
    );
    await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.goto("/");
    await page.getByRole("button", { name: "Accounts" }).click();
    await expect(page.getByTestId("beta-matrix-table")).toBeVisible();
    // Real table semantics: scoped column + row headers.
    await expect(page.getByTestId("beta-matrix-table").locator("th[scope='col']").first()).toBeVisible();
    const rowHeader = page.getByTestId("beta-matrix-row").first().locator("th[scope='row']");
    await expect(rowHeader).toContainText("→");
    // Status in text, never colour-only.
    await expect(page.getByTestId("beta-matrix-row").first()).toContainText(
      "Beta — not yet validated",
    );
    const scan = await new AxeBuilder({ page })
      .include("[data-testid='beta-matrix']")
      .analyze();
    const serious = scan.violations.filter((v) =>
      ["serious", "critical"].includes(v.impact ?? ""),
    );
    if (serious.length > 0) {
      await page.waitForTimeout(400);
      const recheck = await new AxeBuilder({ page })
        .include("[data-testid='beta-matrix']")
        .analyze();
      const still = recheck.violations.filter((v) =>
        ["serious", "critical"].includes(v.impact ?? ""),
      );
      expect(still).toHaveLength(0);
    }
    expect(serious).toHaveLength(0);
  });
}

/**
 * AJ-E301 a11y gate: the wizard dialog — role=dialog + aria-modal +
 * labelled, axe 0 critical/serious on Step 0 in both themes, the
 * keyboard walkthrough (Tab cycles inside the trap, the not-installed
 * row opens install help, Esc closes only when idle), and focus lands
 * on the primary button after a step change.
 */
for (const theme of ["dark", "light"] as const) {
  test(`wizard step0 — dialog semantics, axe 0, keyboard — ${theme}`, async ({ page }) => {
    await page.addInitScript(
      ({ fixture, installs, t0, theme: t }) => {
        const w = window as unknown as Record<string, unknown>;
        w.__AIJ_FIXTURE__ = fixture;
        w.__AIJ_PROVIDER_INSTALLS__ = installs;
        localStorage.setItem("aij.theme", t);
        localStorage.setItem("aij.onboarding.dismissed", "1");
        sessionStorage.setItem("aij.ready.skipped", "1");
        Date.now = () => t0;
      },
      {
        fixture: honestEmpty,
        theme,
        t0: 1_728_800_000_000,
        installs: [
          {
            provider: "claude-code",
            display_name: "Claude Code",
            install_url: "https://claude.com/download",
            status: "not_installed",
            version: null,
            binary: null,
            notes: [],
          },
          {
            provider: "codex",
            display_name: "Codex",
            install_url: "https://developers.openai.com/codex",
            status: "installed",
            version: "0.9.4",
            binary: "/opt/homebrew/bin/codex",
            notes: [],
          },
        ],
      },
    );
    await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.goto("/");
    await page.getByRole("button", { name: "Accounts" }).click();
    await page.getByRole("button", { name: /Add account|Finish setting up/ }).first().click();
    const dialog = page.getByTestId("onboarding-wizard");
    await expect(dialog).toBeVisible();
    await expect(dialog).toHaveAttribute("role", "dialog");
    await expect(dialog).toHaveAttribute("aria-modal", "true");

    // The gate: Continue disabled with the missing provider; the row
    // opens install help keyboard-first.
    await expect(page.getByTestId("install-continue")).toBeDisabled();
    await page.locator("[data-testid='provider-row'] button").first().focus();
    await page.keyboard.press("Enter");
    await expect(page.getByTestId("install-help")).toContainText("Install Claude Code →");

    const scan = await new AxeBuilder({ page })
      .include("[data-testid='onboarding-wizard']")
      .analyze();
    const serious = scan.violations.filter((v) =>
      ["serious", "critical"].includes(v.impact ?? ""),
    );
    if (serious.length > 0) {
      await page.waitForTimeout(400);
      const recheck = await new AxeBuilder({ page })
        .include("[data-testid='onboarding-wizard']")
        .analyze();
      const still = recheck.violations.filter((v) =>
        ["serious", "critical"].includes(v.impact ?? ""),
      );
      expect(still).toHaveLength(0);
    }
    expect(serious).toHaveLength(0);

    // Tab stays inside the dialog (the trap): after several tabs the
    // focused element is still a descendant.
    for (let i = 0; i < 12; i += 1) {
      await page.keyboard.press("Tab");
    }
    await expect(dialog.locator(":focus")).toHaveCount(1);

    // Esc closes when idle.
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
  });
}

/**
 * AJ-E303 a11y gate: the candidate cards — confidence words + login
 * state + activity in TEXT (never colour alone), descriptive
 * aria-labels on the import actions, the managed card disabled with
 * its inline label, and axe 0 critical/serious on the Detect step.
 */
for (const theme of ["dark", "light"] as const) {
  test(`candidate cards — text tiers, aria labels, axe 0 — ${theme}`, async ({ page }) => {
    await page.addInitScript(
      ({ fixture, theme: t }) => {
        const w = window as unknown as Record<string, unknown>;
        w.__AIJ_FIXTURE__ = fixture;
        w.__AIJ_PROVIDER_INSTALLS__ = [
          {
            provider: "claude-code",
            display_name: "Claude Code",
            install_url: "https://claude.com/download",
            status: "installed",
            version: "2.1.296",
            binary: "/usr/local/bin/claude",
            notes: [],
          },
        ];
        w.__AIJ_SCAN_REPORT__ = {
          installs: [
            {
              harness: "claude-code",
              path: "/Users/you/.claude-work",
              origin: "home-pattern",
              login: "logged_in",
              bytes: 9,
              last_used_secs: Math.floor(Date.now() / 1000) - 86_400,
              managed: false,
              notes: [],
              inference: {
                purpose: "Work",
                purpose_confidence: "likely",
                login: "signed-in",
                login_confidence: "found",
                activity: "active",
                activity_confidence: "found",
                activity_words: "Active this week",
                keychain_bound: false,
              },
              signals: ["home-pattern"],
            },
            {
              harness: "claude-code",
              path: "/Users/you/.claude-managed",
              origin: "home-pattern",
              login: "logged_in",
              bytes: 1,
              last_used_secs: Math.floor(Date.now() / 1000) - 86_400,
              managed: true,
              notes: [],
              inference: {
                purpose: "Managed",
                purpose_confidence: "guessed",
                login: "signed-in",
                login_confidence: "found",
                activity: "active",
                activity_confidence: "found",
                activity_words: "Active this week",
                keychain_bound: false,
              },
              signals: ["home-pattern"],
            },
          ],
          probes: [],
          seeds: [],
          desktop_apps: [],
          cancelled: false,
        };
        localStorage.setItem("aij.theme", t);
        localStorage.setItem("aij.onboarding.dismissed", "1");
        sessionStorage.setItem("aij.ready.skipped", "1");
      },
      { fixture: honestEmpty, theme },
    );
    await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.goto("/");
    await page.getByRole("button", { name: "Accounts" }).click();
    await page.getByRole("button", { name: /Add account|Finish setting up/ }).first().click();
    await page.getByTestId("install-continue").click();
    const cards = page.getByTestId("candidate-cards");
    await expect(cards).toBeVisible();
    // Words, not colours: the tier + login + activity all in text.
    await expect(cards).toContainText("Likely: Work");
    await expect(cards).toContainText("Signed in");
    await expect(cards).toContainText("Active this week");
    // Descriptive aria-label on the import action.
    await expect(cards.getByTestId("import-as").first()).toHaveAttribute(
      "aria-label",
      /Import .* as “Work”/,
    );
    // The managed card is disabled inline with its label.
    const managedImport = cards.locator(
      "[data-testid='candidate-card'][data-managed='true'] [data-testid='import-as']",
    );
    await expect(managedImport).toBeDisabled();
    await expect(managedImport).toContainText("Already in AI Juggler");
    const scan = await new AxeBuilder({ page })
      .include("[data-testid='onboarding-wizard']")
      .analyze();
    const serious = scan.violations.filter((v) =>
      ["serious", "critical"].includes(v.impact ?? ""),
    );
    if (serious.length > 0) {
      await page.waitForTimeout(400);
      const recheck = await new AxeBuilder({ page })
        .include("[data-testid='onboarding-wizard']")
        .analyze();
      const still = recheck.violations.filter((v) =>
        ["serious", "critical"].includes(v.impact ?? ""),
      );
      expect(still).toHaveLength(0);
    }
    expect(serious).toHaveLength(0);
  });
}

/**
 * AJ-E304 a11y gate: the recommended-set proposal reads as a list —
 * every checkbox carries a descriptive aria-label (label + rationale),
 * the item rows are list items, and the accept-progress list announces
 * itself politely. Axe 0 critical/serious on the Detect step in BOTH
 * themes (D-16), confirmatory re-scan per AJ-E806.
 */
for (const theme of ["dark", "light"] as const) {
  test(`recommendation proposal — list semantics, aria labels, axe 0 — ${theme}`, async ({
    page,
  }) => {
    await page.addInitScript(
      ({ fixture, theme: t }) => {
        const w = window as unknown as Record<string, unknown>;
        w.__AIJ_FIXTURE__ = fixture;
        w.__AIJ_PROVIDER_INSTALLS__ = [
          {
            provider: "claude-code",
            display_name: "Claude Code",
            install_url: "https://claude.com/download",
            status: "installed",
            version: "2.1.296",
            binary: "/usr/local/bin/claude",
            notes: [],
          },
        ];
        w.__AIJ_SCAN_REPORT__ = {
          installs: [],
          probes: [],
          seeds: [],
          desktop_apps: [],
          cancelled: false,
        };
        w.__AIJ_RECOMMENDATION__ = {
          items: [
            {
              provider: "claude-code",
              label: "Work",
              action: "import",
              rationale: "Signed in — import keeps history",
              default_on: true,
              path: "/Users/you/.claude-work",
            },
            {
              provider: "claude-code",
              label: "Lab",
              action: "import",
              rationale: "Dormant — include when you want it",
              default_on: false,
              path: "/Users/you/.claude-lab",
            },
          ],
          summary: "Your juggling set: Claude ×1 (Work — signed in).",
          start_here: "-users-dev-api",
        };
        localStorage.setItem("aij.theme", t);
        localStorage.setItem("aij.onboarding.dismissed", "1");
        sessionStorage.setItem("aij.ready.skipped", "1");
      },
      { fixture: honestEmpty, theme },
    );
    await page.emulateMedia({ reducedMotion: "reduce", colorScheme: theme });
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.goto("/");
    await page.getByRole("button", { name: "Accounts" }).click();
    await page.getByRole("button", { name: /Add account|Finish setting up/ }).first().click();
    await page.getByTestId("install-continue").click();
    await expect(page.getByTestId("proposal-teaser")).toBeVisible();
    // The teaser buttons carry their own names.
    await expect(page.getByTestId("use-recommended")).toHaveAccessibleName(
      "Use recommended setup →",
    );
    // Customise exposes the checkbox list: list items, one per account.
    await page.getByTestId("proposal-customise").click();
    const block = page.getByTestId("proposal-block");
    await expect(block).toBeVisible();
    await expect(page.getByTestId("proposal-item")).toHaveCount(2);
    const toggles = page.getByTestId("proposal-toggle");
    await expect(toggles).toHaveCount(2);
    // Descriptive labels — label AND rationale, not "import".
    await expect(toggles.nth(0)).toHaveAccessibleName(
      "Work: Signed in — import keeps history",
    );
    await expect(toggles.nth(1)).toHaveAccessibleName(
      "Lab: Dormant — include when you want it",
    );
    await expect(page.getByTestId("accept-setup")).toHaveAccessibleName(
      /Accept setup → \(1\)/,
    );
    // AJ-E806: one confirmatory re-scan after a settle beat — a load
    // race (theme mid-application) doesn't survive it; a real violation
    // reproduces deterministically and still fails the gate.
    const confirmed = await scanSeriousWithConfirm(page);
    expect(confirmed).toHaveLength(0);
  });
}
