/**
 * AJ-E301 vitest — the parameterised wizard's Step-0 install gate:
 * one row per Provider from the probe, the not-installed gate (the
 * Continue button never advances, install help shows the vendor link
 * and "Continue without it"), the installed path (Continue advances
 * to the scan), the coded-error render (user_message + retry, never
 * a raw OS string), and the dialog's role/aria contract.
 */
import { act, fireEvent, render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { OnboardingWizard } from "../screens/OnboardingWizard";
import * as bridgeModule from "../lib/bridge";
import type { ProviderInstallData } from "../lib/bridge";

const w = () => window as unknown as Record<string, unknown>;

beforeEach(() => {
  delete w().__AIJ_PROVIDER_INSTALLS__;
  delete w().__AIJ_ENVIRONMENT_SCAN__;
});

const INSTALLED: ProviderInstallData[] = [
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
const MISSING: ProviderInstallData[] = [
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
];

const mount = (extra?: { onClose?: () => void }) =>
  render(
    <OnboardingWizard
      provider="claude-code"
      onClose={extra?.onClose ?? (() => {})}
      onAccountCreated={() => {}}
      openLoginTerminal={() => {}}
    />,
  );

describe("AJ-E301 Step-0 install gate", () => {
  it("renders one row per Provider with its honest status", async () => {
    w().__AIJ_PROVIDER_INSTALLS__ = MISSING;
    const { container } = mount();
    await waitFor(() =>
      expect(container.querySelectorAll('[data-testid="provider-row"]').length).toBe(2),
    );
    const claudeRow = container.querySelector(
      "[data-testid='provider-row'][data-status='not_installed']",
    )!;
    expect(claudeRow.textContent).toContain("Not installed");
    const codexRow = container.querySelector(
      "[data-testid='provider-row'][data-status='installed']",
    )!;
    expect(codexRow.textContent).toContain("Installed v0.9.4");
  });

  it("a missing binary gates: Continue stays disabled, install help offers the vendor page + Continue without it", async () => {
    w().__AIJ_PROVIDER_INSTALLS__ = MISSING;
    const { container } = mount();
    await waitFor(() =>
      expect(container.querySelector("[data-testid='install-continue']")).not.toBeNull(),
    );
    const continueButton = container.querySelector(
      "[data-testid='install-continue']",
    ) as HTMLButtonElement;
    expect(continueButton.disabled).toBe(true);
    // The row click opens the install help — never advances.
    fireEvent.click(container.querySelector("[data-testid='provider-row'] button")!);
    const help = container.querySelector("[data-testid='install-help']")!;
    expect(help.textContent).toContain("Claude Code isn't installed on this Mac");
    const vendorLink = help.querySelector("a[href='https://claude.com/download']")!;
    expect(vendorLink.textContent).toContain("Install Claude Code →");
    expect(help.textContent).toContain("Continue without it");
    // Identity never became reachable.
    expect(container.querySelector("input[placeholder*='Work']")).toBeNull();
  });

  it("an installed Provider advances to the scan step", async () => {
    w().__AIJ_PROVIDER_INSTALLS__ = INSTALLED;
    const { container } = mount();
    await waitFor(() =>
      expect(container.querySelector("[data-testid='install-continue']")).not.toBeNull(),
    );
    await act(async () => {
      fireEvent.click(container.querySelector("[data-testid='install-continue']")!);
    });
    await waitFor(() =>
      expect(container.textContent).toContain("Let's look at what Claude Code already has"),
    );
  });

  it("the dialog is a role=dialog with an aria-label; Esc closes when idle", async () => {
    w().__AIJ_PROVIDER_INSTALLS__ = INSTALLED;
    let closed = false;
    const { container } = mount({ onClose: () => (closed = true) });
    const dialog = container.querySelector("[data-testid='onboarding-wizard']")!;
    expect(dialog.getAttribute("role")).toBe("dialog");
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    await waitFor(() =>
      expect(container.querySelector("[data-testid='install-continue']")).not.toBeNull(),
    );
    // The label upgrades to the display name once the probe lands.
    expect(dialog.getAttribute("aria-label")).toContain("Claude Code");
    fireEvent.keyDown(window, { key: "Escape" });
    expect(closed).toBe(true);
  });

  it("coded errors render user_message with a retry — never a raw OS string", async () => {
    w().__AIJ_PROVIDER_INSTALLS__ = INSTALLED;
    // The bridge's createAccount rejects with a CODED error (the
    // backend wire shape) — the wizard must render user_message.
    const spy = vi.spyOn(bridgeModule, "createAccount").mockRejectedValue({
      code: "io",
      retryable: true,
      user_message: "Couldn't create the account's private folder. Nothing was half-made.",
    });
    const { container } = mount();
    await waitFor(() =>
      expect(container.querySelector("[data-testid='install-continue']")).not.toBeNull(),
    );
    await act(async () => {
      fireEvent.click(container.querySelector("[data-testid='install-continue']")!);
    });
    await waitFor(() =>
      expect(container.textContent).toContain("Let's look at what Claude Code already has"),
    );
    // Nothing to import (scan empty) → continue with a fresh account.
    await act(async () => {
      fireEvent.click(
        Array.from(container.querySelectorAll("button")).find((button) =>
          button.textContent?.includes("fresh account"),
        )!,
      );
    });
    const input = await waitFor(() => {
      const field = container.querySelector(
        "input[placeholder*='Work']",
      ) as HTMLInputElement;
      expect(field).not.toBeNull();
      return field;
    });
    fireEvent.change(input, { target: { value: "Work" } });
    await act(async () => {
      fireEvent.click(
        Array.from(container.querySelectorAll("button")).find((button) =>
          button.textContent?.includes("Create account"),
        )!,
      );
    });
    await waitFor(() =>
      expect(container.querySelector("[data-testid='wizard-error']")).not.toBeNull(),
    );
    const errorBox = container.querySelector("[data-testid='wizard-error']")!;
    expect(errorBox.textContent).toContain("Couldn't create the account's private folder");
    expect(errorBox.textContent).toContain("Try again");
    spy.mockRestore();
  });
});
