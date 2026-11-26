import { test, expect } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../../", import.meta.url));
const projectPath = root + "examples/micro-8.json";
const project = JSON.parse(readFileSync(projectPath, "utf8"));
const silicon =
  process.env.SILICON_CLI ??
  root + "target/debug/silicon" + (process.platform === "win32" ? ".exe" : "");
const run = (...args: string[]) =>
  JSON.parse(execFileSync(silicon, ["--json", ...args], { encoding: "utf8" }));
const wave = run("simulate", projectPath, "--ticks", "16");
const final = run("run", projectPath, "--memory", "240");
const trace = run("trace", projectPath, "--history", "128");

test("edit a schematic, inspect measured signals and drive debugger controls", async ({
  page,
}) => {
  await page.addInitScript(
    ({ project, wave, final, trace }) => {
      const internals = window as unknown as {
        __TAURI_INTERNALS__: {
          invoke: (
            command: string,
            args: Record<string, unknown>,
          ) => Promise<unknown>;
        };
      };
      let saved = project;
      let stepped = false;
      internals.__TAURI_INTERNALS__ = {
        invoke: async (command, args) => {
          switch (command) {
            case "template":
              return project;
            case "save_project":
              saved = JSON.parse(args.project as string);
              return true;
            case "open_project":
              return saved;
            case "simulate":
              return wave;
            case "prepare":
              return trace.trace.map(
                (item: { instruction: string }) => item.instruction,
              );
            case "assemble":
              throw "2:1: unknown mnemonic ERROR";
            case "debugger":
              if (args.action === "step") stepped = true;
              if (args.action === "reverse") stepped = false;
              return {
                ...final,
                trace: trace.trace,
                console: "",
                stack: [],
                stop: args.action === "run" ? "halted" : null,
                reversed: args.action === "reverse",
                state:
                  args.action === "run"
                    ? final.state
                    : {
                        ...final.state,
                        halted: false,
                        cycles: stepped ? 1 : 0,
                        pc: stepped ? 2 : 0,
                      },
                memory_base: 240,
              };
            default:
              throw Error(command);
          }
        },
      };
    },
    { project, wave, final, trace },
  );
  await page.goto("/");
  await expect(
    page.locator('canvas[aria-label="Circuit schematic"]'),
  ).toBeVisible();
  await page.getByRole("button", { name: "input", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Inspector" })).toBeVisible();
  await expect(page.getByText("7 components")).toBeVisible();
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(page.getByText("6 components")).toBeVisible();
  await page.getByRole("button", { name: "Redo", exact: true }).click();
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByText("Project saved")).toBeVisible();
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await page.getByRole("button", { name: "Open", exact: true }).click();
  await expect(page.getByText("7 components")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Undo", exact: true }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "Simulate", exact: true }).click();
  await page.getByRole("button", { name: "View signals" }).click();
  await expect(
    page.locator('canvas[aria-label="Signal waveforms"]'),
  ).toBeVisible();
  await page.getByRole("button", { name: "Find edge" }).click();
  await expect(page.getByRole("spinbutton", { name: "Cursor" })).toHaveValue(
    "1",
  );
  await page.getByRole("button", { name: "Execution Lab" }).click();
  await page.getByRole("button", { name: "Step F10" }).click();
  await expect(page.locator(".registers")).toContainText("00000002");
  await page.getByRole("button", { name: "Reverse", exact: true }).click();
  await expect(page.locator(".registers")).toContainText("PC 00000000");
  await page.getByRole("button", { name: "Run F5" }).click();
  await expect(page.locator(".hex").first()).toContainText("15");
  await page.getByRole("button", { name: "Assembly", exact: true }).click();
  await expect(page.locator(".monaco-editor")).toBeVisible();
  await page.getByRole("button", { name: "Assemble", exact: true }).click();
  await expect(page.locator("footer").getByRole("alert")).toContainText(
    "unknown mnemonic ERROR",
  );
  await page.keyboard.press("Control+k");
  await expect(
    page.getByRole("dialog", { name: "Command palette" }),
  ).toBeVisible();
});
