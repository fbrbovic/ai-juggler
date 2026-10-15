import { describe, expect, it } from "vitest";
import { fireEvent, render, waitFor, within } from "@testing-library/react";
import {
  CleanupCentre,
  type CleanupBridge,
  type CleanupInventoryData,
} from "../screens/CleanupCentre";
import { ProcessRow, type CleanupRowInfo } from "./ProcessRow";

function row(overrides: Partial<CleanupRowInfo> = {}): CleanupRowInfo {
  return {
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
      first_seen: 1_728_800_000_000,
      last_seen: 1_728_809_000_000,
    },
    ...overrides,
  };
}

function data(overrides: Partial<CleanupInventoryData> = {}): CleanupInventoryData {
  return {
    groups: [
      {
        session: "sess-orphan",
        provider: "claude-code",
        account: "Work Max",
        project: "web",
        machine: "machine-local",
        healthy: false,
        rows: [row()],
      },
      {
        session: "sess-clean",
        provider: "claude-code",
        account: "Team Max",
        project: "api",
        machine: "machine-local",
        healthy: true,
        rows: [
          row({
            pid: 4400,
            state: "running",
            state_label: "Running",
            ports: null,
            cmdline: "node vite",
            evidence: {
              root_pid: 4400,
              parent_pid: null,
              pgid: 4400,
              escaped_group: false,
              verification: { verdict: "verified", detail: "match" },
              first_seen: 1,
              last_seen: 2,
            },
          }),
        ],
      },
    ],
    clean_instances: 2,
    ledger_degraded: false,
    ...overrides,
  };
}

describe("CleanupCentre (AJ-E506)", () => {
  it("renders groups with the verbatim pass2 §2 state labels", async () => {
    const bridge: CleanupBridge = { load: () => Promise.resolve(data()) };
    const view = render(<CleanupCentre bridge={bridge} />);
    await waitFor(() => expect(view.getByTestId("cleanup-group-sess-orphan")).toBeTruthy());
    expect(view.getByTestId("cleanup-row-8123").textContent ?? "").toContain(
      "Still running after its Instance closed",
    );
    // Icon+text badge (never colour alone).
    expect(view.getByTestId("cleanup-state-8123").textContent ?? "").toMatch(/⚑/);
    // Attribution columns.
    expect(view.getByTestId("cleanup-group-sess-orphan").textContent ?? "").toContain(
      "claude-code · Work Max · web",
    );
    expect(view.getByTestId("cleanup-row-8123").textContent ?? "").toContain(":5173");
    view.unmount();
  });

  it("ships ZERO kill controls — no stop/kill buttons anywhere in the tree", async () => {
    const bridge: CleanupBridge = { load: () => Promise.resolve(data()) };
    const view = render(<CleanupCentre bridge={bridge} />);
    await waitFor(() => expect(view.getByTestId("cleanup-group-sess-orphan")).toBeTruthy());
    const killish = view.container.textContent?.toLowerCase() ?? "";
    for (const banned of ["stop process", "kill", "terminate", "free this port"]) {
      expect(killish).not.toContain(banned);
    }
    // The only buttons: group/toggle/filter controls.
    const described = view.container.querySelectorAll("button");
    for (const button of Array.from(described)) {
      const label = button.textContent?.toLowerCase() ?? "";
      expect(label).not.toMatch(/stop|kill|terminate/);
    }
    view.unmount();
  });

  it("collapses healthy trees by default and expands the evidence chain on demand", async () => {
    const bridge: CleanupBridge = { load: () => Promise.resolve(data()) };
    const view = render(<CleanupCentre bridge={bridge} />);
    await waitFor(() => expect(view.getByTestId("cleanup-group-sess-clean")).toBeTruthy());
    // Healthy group collapsed: its running row is hidden.
    expect(view.queryByTestId("cleanup-row-4400")).toBeNull();
    // Orphan group expanded by default; the expansion is offered
    // (title affordance) and opens the evidence panel.
    const toggle = view
      .getByTestId("cleanup-row-8123")
      .querySelector("button") as HTMLButtonElement;
    expect(toggle.title).toContain("Why does Juggler think this is mine?");
    fireEvent.click(toggle);
    expect(view.getByTestId("cleanup-evidence-8123").textContent ?? "").toContain(
      "Why does Juggler think this is mine?",
    );
    view.unmount();
  });

  it("renders the copy-exact empty success and the degraded banner", async () => {
    const empty: CleanupBridge = {
      load: () =>
        Promise.resolve({ groups: [], clean_instances: 3, ledger_degraded: false }),
    };
    const view = render(<CleanupCentre bridge={empty} />);
    await waitFor(() => expect(view.getByTestId("cleanup-empty")).toBeTruthy());
    expect(view.getByTestId("cleanup-empty").textContent ?? "").toContain(
      "Nothing left behind — 3 Instances running clean.",
    );
    view.unmount();

    const degraded: CleanupBridge = {
      load: () =>
        Promise.resolve({ groups: [], clean_instances: 0, ledger_degraded: true }),
    };
    const view2 = render(<CleanupCentre bridge={degraded} />);
    await waitFor(() => expect(view2.getByTestId("cleanup-degraded-banner")).toBeTruthy());
    expect(view2.getByTestId("cleanup-degraded-banner").textContent ?? "").toContain(
      "Process tracking degraded — cleanup is read-only until restart.",
    );
    view2.unmount();
  });

  it("announces filter changes politely and exposes exactly the four filters", async () => {
    const bridge: CleanupBridge = { load: () => Promise.resolve(data()) };
    const view = render(<CleanupCentre bridge={bridge} />);
    await waitFor(() =>
      expect(view.getByTestId("cleanup-filter-announcement")).toBeTruthy(),
    );
    const group = view.getByRole("radiogroup", { name: "Cleanup filter" });
    const labels = within(group)
      .getAllByRole("radio")
      .map((radio) => radio.textContent);
    expect(labels).toEqual(["Needs attention", "All managed", "Re-adopted", "History (7 days)"]);
    expect(view.getByTestId("cleanup-filter-announcement").textContent ?? "").toContain(
      "Needs attention",
    );
    view.unmount();
  });
});

describe("ProcessRow evidence expansion (AJ-E506 Req.4)", () => {
  it("shows the complete chain: root, ppid, PGID containment, identity, seen", () => {
    const evidenceRow = row();
    const view = render(
      <ul>
        <ProcessRow row={evidenceRow} />
      </ul>,
    );
    // Rows start collapsed.
    expect(view.queryByTestId("cleanup-evidence-8123")).toBeNull();
    fireEvent.click(view.getByTestId("cleanup-row-8123").querySelector("button")!);
    const panel = view.getByTestId("cleanup-evidence-8123");
    const text = panel.textContent ?? "";
    expect(text).toContain("Instance root PID 8100");
    expect(text).toContain("first-seen parent 8100");
    expect(text).toContain("matches the root (contained)");
    expect(text).toContain("verified — start-time and exe match the live process");
    expect(text).toContain("Read-only (Tier 0)");
    view.unmount();
  });

  it("labels escaped groups honestly in the chain", async () => {
    const escaped = row({
      pid: 9100,
      evidence: {
        root_pid: 8100,
        parent_pid: 8100,
        pgid: 9100,
        escaped_group: true,
        verification: { verdict: "unverifiable", detail: "live start-time differs" },
        first_seen: 1,
        last_seen: 2,
      },
      state: "unverifiable",
      state_label: "Unverifiable — will not be touched",
    });
    const view = render(
      <ul>
        <ProcessRow row={escaped} />
      </ul>,
    );
    expect(view.getByTestId("cleanup-state-9100").textContent ?? "").toContain(
      "Unverifiable — will not be touched",
    );
    fireEvent.click(view.getByTestId("cleanup-row-9100").querySelector("button")!);
    expect(view.getByTestId("cleanup-evidence-9100").textContent ?? "").toContain(
      "escaped its process group",
    );
    view.unmount();
  });
});
