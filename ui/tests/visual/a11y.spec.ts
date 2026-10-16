import { expect, test } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
import { allStatuses, type FixtureSet } from "./fixtures";

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
