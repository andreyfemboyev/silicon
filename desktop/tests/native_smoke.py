"""Check a packaged SILICON binary through a running tauri-driver instance."""

import argparse
import json
import subprocess
import tempfile
import time
from pathlib import Path
from urllib.error import HTTPError
from urllib.request import Request, urlopen


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("application", type=Path)
    parser.add_argument("--driver", default="http://127.0.0.1:4444")
    parser.add_argument("--gtk-dialogs", action="store_true", help="check save and reopen via xdotool on an isolated X11 display")
    args = parser.parse_args()

    def request(path, data=None, method=None):
        payload = json.dumps(data).encode() if data is not None else None
        headers = {"Content-Type": "application/json"} if payload is not None else {}
        try:
            with urlopen(Request(args.driver + path, payload, headers, method=method), timeout=30) as response:
                return json.load(response)["value"]
        except HTTPError as error:
            raise RuntimeError(error.read().decode()) from error

    session = request("/session", {
        "capabilities": {"alwaysMatch": {"tauri:options": {
            "application": str(args.application.resolve()),
        }}},
    })["sessionId"]
    root = "/session/" + session

    def js(source, *arguments):
        return request(root + "/execute/sync", {"script": source, "args": arguments})

    def wait_for(source, description):
        deadline = time.monotonic() + 16
        while time.monotonic() < deadline:
            if js(source):
                return
            time.sleep(0.1)
        status = js("return document.querySelector('footer')?.innerText ?? document.body.innerText")
        raise AssertionError(f"{description}; page reported {status!r}")

    def button(text, selector="button"):
        assert js("""
            const button = [...document.querySelectorAll(arguments[1])]
              .find(button => button.textContent.trim().startsWith(arguments[0]));
            if (!button || button.disabled) return false;
            button.click();
            return true;
        """, text, selector), text

    def choose_file(title, path):
        window = subprocess.check_output(
            ["xdotool", "search", "--sync", "--onlyvisible", "--name", f"^{title}$"],
            text=True,
            timeout=10,
        ).splitlines()[0]
        time.sleep(0.8)
        subprocess.run(["xdotool", "windowfocus", window, "key", "ctrl+l"], check=True)
        time.sleep(0.2)
        subprocess.run(["xdotool", "key", "ctrl+a"], check=True)
        subprocess.run(["xdotool", "type", "--clearmodifiers", "--delay", "15", str(path)], check=True)
        time.sleep(0.5)
        subprocess.run(["xdotool", "key", "Return"], check=True)
        if title == "Open File":
            time.sleep(0.6)
            visible = subprocess.run(
                ["xdotool", "search", "--onlyvisible", "--name", f"^{title}$"],
                capture_output=True,
                check=False,
            )
            if visible.returncode == 0:
                subprocess.run(["xdotool", "key", "Return"], check=True)

    try:
        wait_for("return !!document.querySelector('canvas[aria-label=\"Circuit schematic\"]')", "Schematic did not render")
        button("Simulate")
        wait_for("return document.querySelector('footer')?.innerText.includes('transitions')", "Circuit did not produce transitions")
        button("View signals")
        wait_for("return !!document.querySelector('canvas[aria-label=\"Signal waveforms\"]')", "Waveforms did not render")
        button("Execution Lab", "nav button")
        button("Step")
        wait_for("return document.querySelector('.registers')?.innerText.includes('00000002')", "PC did not step")
        button("Reverse")
        wait_for("return document.querySelector('.registers code strong')?.innerText === '00000000'", "PC did not reverse")
        button("Run")
        wait_for("return document.querySelector('footer')?.innerText.includes('Halted')", "Program did not halt")
        button("Assembly", "nav button")
        wait_for("return !!document.querySelector('.monaco-editor')", "Bundled Monaco did not load")
        button("Assemble")
        wait_for("return document.querySelector('footer')?.innerText.includes('Assembled')", "Assembly did not succeed")
        print("Native schematic, waveform, step, reverse, run and Monaco checks passed.", flush=True)
        if args.gtk_dialogs:
            with tempfile.TemporaryDirectory(prefix="silicon-smoke-") as folder:
                project = Path(folder) / "native-test.silicon.json"
                button("Save", ".top-actions button")
                choose_file("Save File", project)
                wait_for("return document.querySelector('footer')?.innerText.includes('Project saved')", "Save dialog failed")
                assert json.loads(project.read_text())["architecture"]["name"] == "MICRO-8"
                button("Open", ".top-actions button")
                choose_file("Open File", project)
                wait_for("return document.querySelector('footer')?.innerText.includes('Project opened')", "Open dialog failed")
                print("Native save and reopen checks passed.", flush=True)
    finally:
        request(root, method="DELETE")


if __name__ == "__main__":
    main()
