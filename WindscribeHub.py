from __future__ import annotations
import ctypes, json, os, queue, secrets, string, subprocess, threading, uuid, winreg
from ctypes import wintypes
from datetime import datetime
from pathlib import Path
from tkinter import messagebox
import customtkinter as ctk

APP_NAME = "Windscribe Pro"
APP_VERSION = "0.0.1"
PACKAGE_ID = "Windscribe.Windscribe"
P = {
    "bg": "#07111F", "sidebar": "#09182B", "panel": "#0D2038", "card": "#10243D",
    "border": "#203C61", "blue": "#087CFF", "purple": "#704AE8",
    "green": "#19C47D", "amber": "#F2B94B", "text": "#F4F7FD",
    "muted": "#A5B5CE", "quiet": "#7185A5", "console": "#07111D"
}
NO_WINDOW = getattr(subprocess, "CREATE_NO_WINDOW", 0)
REGISTRY_KEYS = {
    "one": r"HKCU\Software\Microsoft\Windows\CurrentVersion\NetControlPort",
    "two": r"HKCU\Software\Windscribe\Windscribe2"
}

def run(args, timeout=45):
    return subprocess.run(args, capture_output=True, text=True, errors="replace",
                          timeout=timeout, creationflags=NO_WINDOW)

def dpapi(data: bytes, protect: bool) -> bytes:
    if os.name != "nt":
        raise OSError("Windows DPAPI is required for the encrypted account vault.")
    class BLOB(ctypes.Structure):
        _fields_ = [("cbData", wintypes.DWORD), ("pbData", ctypes.POINTER(ctypes.c_byte))]
    crypt = ctypes.WinDLL("Crypt32", use_last_error=True)
    kernel = ctypes.WinDLL("Kernel32", use_last_error=True)
    buf = ctypes.create_string_buffer(data)
    src = BLOB(len(data), ctypes.cast(buf, ctypes.POINTER(ctypes.c_byte)))
    dst = BLOB()
    free = kernel.LocalFree
    free.argtypes = [ctypes.c_void_p]
    free.restype = ctypes.c_void_p
    if protect:
        fn = crypt.CryptProtectData
        fn.argtypes = [ctypes.POINTER(BLOB), wintypes.LPCWSTR, ctypes.c_void_p,
                       ctypes.c_void_p, ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(BLOB)]
        ok = fn(ctypes.byref(src), "Windscribe Pro account vault", None, None, None, 1, ctypes.byref(dst))
        desc = None
    else:
        fn = crypt.CryptUnprotectData
        desc = wintypes.LPWSTR()
        fn.argtypes = [ctypes.POINTER(BLOB), ctypes.POINTER(wintypes.LPWSTR),
                       ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(BLOB)]
        ok = fn(ctypes.byref(src), ctypes.byref(desc), None, None, None, 1, ctypes.byref(dst))
    if not ok:
        code = ctypes.get_last_error()
        raise OSError(code, ctypes.FormatError(code))
    try:
        return ctypes.string_at(dst.pbData, dst.cbData)
    finally:
        if dst.pbData:
            free(dst.pbData)
        if desc and desc.value:
            free(ctypes.cast(desc, ctypes.c_void_p))

def find_windscribe_exe():
    candidates = []
    for hive in (winreg.HKEY_LOCAL_MACHINE, winreg.HKEY_CURRENT_USER):
        for root in (r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
                     r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"):
            try:
                with winreg.OpenKey(hive, root) as parent:
                    for i in range(winreg.QueryInfoKey(parent)[0]):
                        try:
                            with winreg.OpenKey(parent, winreg.EnumKey(parent, i)) as key:
                                name = str(winreg.QueryValueEx(key, "DisplayName")[0])
                                if "windscribe" not in name.casefold():
                                    continue
                                for field in ("DisplayIcon", "InstallLocation"):
                                    try:
                                        raw = str(winreg.QueryValueEx(key, field)[0]).strip('"').split(",")[0]
                                        item = Path(os.path.expandvars(raw))
                                        candidates.extend([item, item / "Windscribe.exe"])
                                    except OSError:
                                        pass
                        except OSError:
                            pass
            except OSError:
                pass
    pf = Path(os.environ.get("ProgramFiles", r"C:\Program Files"))
    local = Path(os.environ.get("LOCALAPPDATA", str(Path.home())))
    candidates.extend([pf / "Windscribe" / "Windscribe.exe",
                        local / "Programs" / "Windscribe" / "Windscribe.exe"])
    return next((x for x in candidates if x.is_file() and x.suffix.lower() == ".exe"), None)

class WindscribeHub(ctk.CTk):
    def __init__(self):
        super().__init__()
        self.title(f"{APP_NAME}  v{APP_VERSION}")
        self.geometry("1320x900")
        self.minsize(1080, 760)
        self.configure(fg_color=P["bg"])
        self.events = queue.Queue()
        self.busy = False
        self.installed = False
        self.update_available = False
        self.managed_by_winget = False
        self.running = False
        self.accounts = []
        self.selected_id = None
        self.revealed = set()
        self.password_visible = False
        self.vault = Path(os.environ.get("LOCALAPPDATA", str(Path.home()))) / "WindscribeHub" / "accounts.vault"
        self._set_icon()
        self._build_ui()
        self._load_vault()
        self._render_accounts()
        self.log("Windscribe Pro Hub is ready.")
        self.after(120, self._poll)
        self.refresh_status()

    def font(self, size=12, bold=False):
        return ctk.CTkFont("Segoe UI", size=size, weight="bold" if bold else "normal")

    def label(self, parent, text, size=12, color=None, bold=False, **kw):
        return ctk.CTkLabel(parent, text=text, font=self.font(size, bold),
                            text_color=color or P["text"], **kw)

    def button(self, parent, text, command, color=None, width=140, height=38):
        return ctk.CTkButton(parent, text=text, command=command, width=width, height=height,
            corner_radius=10, fg_color=color or P["blue"],
            hover_color="#0869D8" if color in (None, P["blue"]) else color,
            text_color="white", font=self.font(11, True))

    def card(self, parent, color=None):
        return ctk.CTkFrame(parent, fg_color=color or P["card"], corner_radius=16,
                            border_width=1, border_color=P["border"])

    def _set_icon(self):
        icon = Path(__file__).parent / "assets" / "WindscribeHub.ico"
        if icon.exists():
            try:
                self.iconbitmap(str(icon))
            except Exception:
                pass

    def _build_ui(self):
        self.grid_columnconfigure(1, weight=1)
        self.grid_rowconfigure(0, weight=1)
        sidebar = ctk.CTkFrame(self, width=232, fg_color=P["sidebar"], corner_radius=0)
        sidebar.grid(row=0, column=0, sticky="nsew")
        sidebar.grid_propagate(False)
        mark = ctk.CTkFrame(sidebar, width=54, height=54, fg_color="#123A67",
                            corner_radius=16, border_width=1, border_color=P["blue"])
        mark.pack(anchor="w", padx=22, pady=(25, 10))
        mark.pack_propagate(False)
        self.label(mark, "W", 29, "#20B9F4", True).place(relx=.5, rely=.5, anchor="center")
        self.label(sidebar, "WINDSCRIBE", 17, P["text"], True).pack(anchor="w", padx=22)
        self.label(sidebar, "PRO  •  DESKTOP HUB", 9, "#20B9F4", True).pack(anchor="w", padx=22, pady=(2, 25))
        self.button(sidebar, "⌂    Overview", lambda: self.area._parent_canvas.yview_moveto(0),
                    "#12345A", 192, 42).pack(padx=18, pady=4)
        self.button(sidebar, "◈    Windscribe Account", self.goto_accounts,
                    "#12345A", 192, 42).pack(padx=18, pady=4)
        self.button(sidebar, "⚙    Registry Tools", self.goto_registry,
                    "#12345A", 192, 42).pack(padx=18, pady=4)
        spacer = ctk.CTkFrame(sidebar, fg_color="transparent")
        spacer.pack(fill="both", expand=True)
        secure = self.card(sidebar, "#0D2038")
        secure.pack(fill="x", padx=15, pady=16)
        self.label(secure, "●  LOCAL & ENCRYPTED", 10, P["green"], True).pack(anchor="w", padx=13, pady=(13, 4))
        self.label(secure, "Credentials protected with\nWindows DPAPI.", 10, P["muted"],
                   justify="left").pack(anchor="w", padx=13, pady=(0, 13))
        self.label(sidebar, f"v{APP_VERSION}  •  Demonariya", 9, P["quiet"]).pack(pady=(0, 17))

        self.area = ctk.CTkScrollableFrame(self, fg_color="transparent", scrollbar_button_color="#315177")
        self.area.grid(row=0, column=1, sticky="nsew", padx=22, pady=18)
        self.area.grid_columnconfigure((0, 1), weight=1, uniform="main")
        head = ctk.CTkFrame(self.area, fg_color="transparent")
        head.grid(row=0, column=0, columnspan=2, sticky="ew", pady=(2, 17))
        head.grid_columnconfigure(0, weight=1)
        self.label(head, "Windscribe Pro", 30, P["text"], True).grid(row=0, column=0, sticky="w")
        self.label(head, "VPN setup, account access and maintenance in one place", 12, P["muted"]).grid(row=1, column=0, sticky="w")
        self.ready = self.label(head, "● CHECKING", 11, P["amber"], True)
        self.ready.grid(row=0, column=1, rowspan=2, padx=8)
        self._status_card()
        self._setup_card()
        self._registry_card()
        self._accounts_card()
        self._activity_card()

    def _status_card(self):
        box = self.card(self.area, "#0D2038")
        box.grid(row=1, column=0, columnspan=2, sticky="ew", pady=(0, 15))
        box.grid_columnconfigure(0, weight=1)
        self.status_title = self.label(box, "Checking Windscribe…", 17, P["text"], True)
        self.status_title.grid(row=0, column=0, sticky="w", padx=18, pady=(15, 2))
        self.status_detail = self.label(box, "Detecting installation and available updates", 10, P["muted"])
        self.status_detail.grid(row=1, column=0, sticky="w", padx=18, pady=(0, 2))
        self.process_label = self.label(box, "App process: checking…", 10, P["muted"])
        self.process_label.grid(row=2, column=0, sticky="w", padx=18, pady=(0, 15))
        self.refresh_button = self.button(box, "Refresh Status", self.refresh_status, "#263E60", 145, 36)
        self.refresh_button.grid(row=0, column=1, rowspan=3, padx=17)

    def _setup_card(self):
        box = self.card(self.area)
        box.grid(row=2, column=0, sticky="nsew", padx=(0, 8), pady=(0, 15))
        self.label(box, "⬇  Windscribe Setup", 16, P["text"], True).pack(anchor="w", padx=16, pady=(16, 5))
        self.label(box, "Install or update the official app with WinGet.", 10, P["muted"]).pack(anchor="w", padx=16, pady=(0, 14))
        self.setup_button = self.button(box, "Checking…", self.install_or_update, P["blue"], 185, 38)
        self.setup_button.pack(anchor="w", padx=16, pady=(0, 9))
        self.open_button = self.button(box, "Open Windscribe", self.open_windscribe, P["green"], 185, 38)
        self.open_button.pack(anchor="w", padx=16, pady=(0, 16))
        self.open_button.configure(state="disabled")

    def _registry_card(self):
        box = self.card(self.area)
        box.grid(row=2, column=1, sticky="nsew", padx=(8, 0), pady=(0, 15))
        self.label(box, "⚙  Registry Cleanup", 16, P["text"], True).pack(anchor="w", padx=16, pady=(16, 5))
        self.label(box, "A .reg backup is exported before deletion.", 10, P["muted"]).pack(anchor="w", padx=16, pady=(0, 14))
        self.button(box, "Run Command 1", lambda: self.run_registry("one"), P["purple"], 165, 36).pack(anchor="w", padx=16, pady=(0, 8))
        self.button(box, "Run Command 2", lambda: self.run_registry("two"), P["purple"], 165, 36).pack(anchor="w", padx=16, pady=(0, 16))

    def _accounts_card(self):
        box = self.card(self.area)
        box.grid(row=3, column=0, columnspan=2, sticky="ew", pady=(0, 15))
        box.grid_columnconfigure((0, 1), weight=1, uniform="accounts")
        left = ctk.CTkFrame(box, fg_color="transparent")
        left.grid(row=0, column=0, sticky="nsew", padx=17, pady=16)
        right = ctk.CTkFrame(box, fg_color="transparent")
        right.grid(row=0, column=1, sticky="nsew", padx=17, pady=16)
        self.label(left, "🔐  Windscribe Account", 16, P["text"], True).pack(anchor="w")
        self.label(left, "Save credentials for your Windscribe logins.", 10, P["muted"]).pack(anchor="w", pady=(3, 13))
        self.label(left, "Username", 11, P["text"], True).pack(anchor="w")
        self.username_entry = ctk.CTkEntry(left, height=38, placeholder_text="Enter Windscribe username", corner_radius=9)
        self.username_entry.pack(fill="x", pady=(5, 9))
        self.label(left, "Password", 11, P["text"], True).pack(anchor="w")
        pwrow = ctk.CTkFrame(left, fg_color="transparent")
        pwrow.pack(fill="x", pady=(5, 9))
        self.password_entry = ctk.CTkEntry(pwrow, height=38, show="•", placeholder_text="Enter Windscribe password", corner_radius=9)
        self.password_entry.pack(side="left", fill="x", expand=True)
        self.button(pwrow, "Show", self.toggle_input, "#263E60", 62, 38).pack(side="left", padx=(6, 0))
        gen = ctk.CTkFrame(left, fg_color="transparent")
        gen.pack(fill="x", pady=(0, 9))
        self.button(gen, "Suggest Username", self.generate_username, "#1E5590", 142, 34).pack(side="left", padx=(0, 6))
        self.button(gen, "Generate Password", self.generate_password, P["purple"], 145, 34).pack(side="left")
        self.button(left, "Save Windscribe Account", self.save_account, P["green"], 190, 38).pack(anchor="w")
        self.label(left, "Username suggestions are not registered automatically.", 9, P["quiet"]).pack(anchor="w", pady=(8, 0))

        self.label(right, "Saved Accounts", 16, P["text"], True).pack(anchor="w")
        self.label(right, "All saved Windscribe logins appear here.", 10, P["muted"]).pack(anchor="w", pady=(3, 10))
        heading = ctk.CTkFrame(right, fg_color="#1A3455", corner_radius=8)
        heading.pack(fill="x", pady=(0, 4))
        self.label(heading, "USERNAME", 9, P["muted"], True).pack(side="left", padx=10, pady=8)
        self.label(heading, "PASSWORD", 9, P["muted"], True).pack(side="right", padx=12, pady=8)
        self.account_list = ctk.CTkScrollableFrame(right, height=220, fg_color="#0B1A2D", corner_radius=9)
        self.account_list.pack(fill="both", expand=True)
        self.account_list.grid_columnconfigure(0, weight=1)
        self.account_count = self.label(right, "0 saved accounts", 9, P["quiet"])
        self.account_count.pack(anchor="w", pady=(7, 0))
        actions = ctk.CTkFrame(right, fg_color="transparent")
        actions.pack(fill="x", pady=(7, 0))
        self.button(actions, "Load", self.load_selected, "#263E60", 62, 32).pack(side="left", padx=(0, 4))
        self.button(actions, "Copy User", self.copy_username, "#263E60", 84, 32).pack(side="left", padx=4)
        self.button(actions, "Copy Pass", self.copy_password, "#263E60", 84, 32).pack(side="left", padx=4)
        self.button(actions, "Delete", self.delete_selected, "#6A293C", 70, 32).pack(side="left", padx=4)

    def _activity_card(self):
        box = self.card(self.area)
        box.grid(row=4, column=0, columnspan=2, sticky="ew", pady=(0, 12))
        box.grid_columnconfigure(0, weight=1)
        bar = ctk.CTkFrame(box, fg_color="transparent")
        bar.pack(fill="x", padx=15, pady=(12, 6))
        self.label(bar, "▤  Activity Log", 14, P["text"], True).pack(side="left")
        self.button(bar, "Clear", self.clear_log, "#263E60", 65, 29).pack(side="right")
        self.log_box = ctk.CTkTextbox(box, height=115, fg_color=P["console"], text_color="#B6C8E6",
                                      corner_radius=9, font=("Consolas", 10))
        self.log_box.pack(fill="x", padx=15, pady=(0, 14))
        self.log_box.configure(state="disabled")

    def _start_worker(self, fn):
        if self.busy:
            return
        self.busy = True
        self.ready.configure(text="● WORKING", text_color=P["amber"])
        self._set_buttons()
        def target():
            try:
                fn()
            except Exception as exc:
                self.events.put(("log", f"ERROR  {exc}"))
            finally:
                self.events.put(("done", None))
        threading.Thread(target=target, daemon=True).start()

    def _poll(self):
        try:
            while True:
                kind, value = self.events.get_nowait()
                if kind == "log":
                    self.log(str(value))
                elif kind == "status":
                    self._apply_status(*value)
                elif kind == "done":
                    self.busy = False
                    self.ready.configure(text="● READY" if self.installed else "● NOT INSTALLED",
                                         text_color=P["green"] if self.installed else P["amber"])
                    self._set_buttons()
        except queue.Empty:
            pass
        self.after(120, self._poll)

    def _set_buttons(self):
        self.refresh_button.configure(state="disabled" if self.busy else "normal")
        if self.busy:
            self.setup_button.configure(state="disabled")
            self.open_button.configure(state="disabled")
        elif not self.installed:
            self.setup_button.configure(text="Install Windscribe", fg_color=P["blue"], state="normal")
            self.open_button.configure(state="disabled")
        elif self.update_available:
            self.setup_button.configure(text="Update Windscribe", fg_color=P["purple"], state="normal")
            self.open_button.configure(state="normal")
        elif not self.managed_by_winget:
            self.setup_button.configure(text="Update check unavailable", fg_color="#394A62", state="disabled")
            self.open_button.configure(state="normal")
        else:
            self.setup_button.configure(text="Windscribe Up to Date", fg_color="#23553F", state="disabled")
            self.open_button.configure(state="normal")

    def _is_running(self):
        try:
            result = run(["tasklist", "/FI", "IMAGENAME eq Windscribe.exe", "/FO", "CSV", "/NH"], 15)
            return "windscribe.exe" in (result.stdout or "").casefold()
        except Exception:
            return False

    def _check_status(self):
        running = self._is_running()
        local_exe = find_windscribe_exe()
        try:
            listed = run(["winget", "list", "--id", PACKAGE_ID, "--exact", "--accept-source-agreements"], 45)
        except FileNotFoundError:
            installed = local_exe is not None
            version = "Detected outside WinGet" if installed else ""
            self.events.put(("status", (installed, False, version, running,
                                        "WinGet is unavailable; update detection is unavailable.", False)))
            return

        output = (listed.stdout or "") + "\n" + (listed.stderr or "")
        managed = listed.returncode == 0 and PACKAGE_ID.casefold() in output.casefold()
        installed = managed or local_exe is not None
        version = ""
        if managed:
            for line in output.splitlines():
                if PACKAGE_ID.casefold() in line.casefold():
                    cols = line.split()
                    if len(cols) > 1:
                        version = cols[-2] if len(cols) > 2 else cols[-1]
                    break
        elif installed:
            version = "Detected outside WinGet"

        upgrade = False
        if managed:
            try:
                result = run(["winget", "upgrade", "--id", PACKAGE_ID, "--exact", "--accept-source-agreements"], 60)
                text = (result.stdout or "") + "\n" + (result.stderr or "")
                low = text.casefold()
                upgrade = result.returncode == 0 and PACKAGE_ID.casefold() in low and not any(
                    x in low for x in ("no applicable upgrade found", "no installed package found", "no available upgrade"))
            except Exception:
                upgrade = False
        self.events.put(("status", (installed, upgrade, version, running, "", managed)))

    def _apply_status(self, installed, upgrade, version, running, error, managed):
        self.installed, self.update_available, self.running = installed, upgrade, running
        self.managed_by_winget = managed
        self.process_label.configure(text=f"App process: {'RUNNING' if running else 'NOT RUNNING'}",
                                     text_color=P["green"] if running else P["muted"])
        if not installed and error:
            title, detail, color = "Windscribe status check incomplete", error, P["amber"]
        elif not installed:
            title, detail, color = "Windscribe is not installed", f"Package ID: {PACKAGE_ID}", P["amber"]
        elif not managed:
            title, detail, color = "Windscribe is installed", "Found on this PC • This installation is not managed by WinGet", P["green"]
        elif upgrade:
            title, detail, color = "Windscribe update available", f"Installed version: {version or 'Detected'}  •  Update available", P["amber"]
        else:
            title, detail, color = "Windscribe is installed", f"Installed version: {version or 'Detected'}  •  Up to date", P["green"]
        self.status_title.configure(text=title, text_color=color)
        self.status_detail.configure(text=detail)
        self._set_buttons()

    def refresh_status(self):
        self._start_worker(self._check_status)

    def install_or_update(self):
        action = "upgrade" if self.installed else "install"
        def work():
            self.events.put(("log", f"Starting WinGet {action}…"))
            args = ["winget", action, "--id", PACKAGE_ID, "--exact", "--source", "winget",
                    "--accept-package-agreements", "--accept-source-agreements"]
            proc = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                    text=True, errors="replace", creationflags=NO_WINDOW)
            if proc.stdout:
                for line in proc.stdout:
                    if line.strip():
                        self.events.put(("log", line.strip()))
            if proc.wait():
                raise RuntimeError(f"WinGet {action} failed.")
            self.events.put(("log", f"WinGet {action} completed successfully."))
            self._check_status()
        self._start_worker(work)

    def open_windscribe(self):
        if not self.installed:
            return
        def work():
            exe = find_windscribe_exe()
            if exe:
                os.startfile(str(exe))
            else:
                roots = [Path(os.environ.get("ProgramData", r"C:\ProgramData")) / "Microsoft/Windows/Start Menu/Programs",
                         Path(os.environ.get("APPDATA", "")) / "Microsoft/Windows/Start Menu/Programs"]
                shortcuts = [p for root in roots if root.exists() for p in root.rglob("*Windscribe*.lnk")]
                if not shortcuts:
                    raise RuntimeError("Windscribe is installed, but its launcher was not found.")
                os.startfile(str(shortcuts[0]))
            self.events.put(("log", "Windscribe launch requested."))
        self._start_worker(work)

    def run_registry(self, key_id):
        key = REGISTRY_KEYS[key_id]
        if not messagebox.askyesno("Confirm Registry Cleanup",
            f"Export a backup, then delete this registry key?\n\n{key}", icon="warning", parent=self):
            return
        def work():
            folder = Path.home() / "Documents" / "Windscribe Hub" / "Backups"
            folder.mkdir(parents=True, exist_ok=True)
            name = "NetControlPort" if key_id == "one" else "Windscribe2"
            backup = folder / f"{name}_{datetime.now():%Y%m%d_%H%M%S}.reg"
            exported = run(["reg.exe", "export", key, str(backup), "/y"])
            if exported.returncode:
                raise RuntimeError("Registry export failed; the key was not deleted.")
            deleted = run(["reg.exe", "delete", key, "/f"])
            if deleted.returncode:
                raise RuntimeError(deleted.stderr.strip() or "Registry deletion failed.")
            self.events.put(("log", f"Deleted {key}; backup saved to {backup}"))
        self._start_worker(work)

    def _load_vault(self):
        if not self.vault.exists():
            return
        try:
            data = json.loads(dpapi(self.vault.read_bytes(), False).decode("utf-8"))
            if isinstance(data, list):
                self.accounts = [x for x in data if isinstance(x, dict) and all(
                    isinstance(x.get(k), str) for k in ("id", "username", "password"))]
            self.log(f"Loaded {len(self.accounts)} saved Windscribe account(s).")
        except Exception as exc:
            self.log(f"Could not decrypt account vault: {exc}")

    def _save_vault(self):
        self.vault.parent.mkdir(parents=True, exist_ok=True)
        raw = json.dumps(self.accounts, ensure_ascii=False).encode("utf-8")
        encrypted = dpapi(raw, True)
        temp = self.vault.with_suffix(".tmp")
        temp.write_bytes(encrypted)
        temp.replace(self.vault)

    def _render_accounts(self):
        for child in self.account_list.winfo_children():
            child.destroy()
        self.account_count.configure(text=f"{len(self.accounts)} saved Windscribe account(s)")
        if not self.accounts:
            self.label(self.account_list, "No saved Windscribe accounts yet.", 11, P["quiet"]).pack(pady=24)
            return
        for index, item in enumerate(self.accounts):
            aid = item["id"]
            row = ctk.CTkFrame(self.account_list, fg_color="#132A46" if index % 2 == 0 else "#0F223A", corner_radius=8)
            row.pack(fill="x", padx=3, pady=3)
            row.grid_columnconfigure(0, weight=2)
            row.grid_columnconfigure(1, weight=1)
            self.label(row, item["username"], 11, P["text"], True).grid(row=0, column=0, sticky="w", padx=9, pady=10)
            shown = aid in self.revealed
            secret = item["password"] if shown else "•" * min(max(len(item["password"]), 8), 15)
            self.label(row, secret, 10, P["muted"]).grid(row=0, column=1, sticky="w", padx=4)
            actions = ctk.CTkFrame(row, fg_color="transparent")
            actions.grid(row=0, column=2, padx=4)
            self.button(actions, "Hide" if shown else "Show", lambda x=aid: self.toggle_row_password(x),
                        "#263E60", 50, 28).pack(side="left", padx=2)
            self.button(actions, "Select", lambda x=aid: self.select_account(x),
                        P["blue"] if aid == self.selected_id else "#263E60", 55, 28).pack(side="left", padx=2)
            self.button(actions, "×", lambda x=aid: self.delete_account(x),
                        "#6A293C", 28, 28).pack(side="left", padx=2)

    def toggle_row_password(self, aid):
        if aid in self.revealed:
            self.revealed.remove(aid)
        else:
            self.revealed.add(aid)
        self._render_accounts()

    def select_account(self, aid):
        self.selected_id = aid
        self._render_accounts()

    def _selected(self):
        return next((x for x in self.accounts if x["id"] == self.selected_id), None)

    def save_account(self):
        username = self.username_entry.get().strip()
        password = self.password_entry.get()
        if not username or not password:
            messagebox.showwarning("Missing details", "Enter both the Windscribe username and password.", parent=self)
            return
        existing = next((x for x in self.accounts if x["username"].casefold() == username.casefold()), None)
        if existing:
            existing["password"] = password
            self.selected_id = existing["id"]
        else:
            item = {"id": uuid.uuid4().hex, "username": username, "password": password}
            self.accounts.insert(0, item)
            self.selected_id = item["id"]
        try:
            self._save_vault()
            self._render_accounts()
            self.log(f"Saved Windscribe credentials for {username}.")
            self.password_entry.delete(0, "end")
            self.password_entry.configure(show="•")
            self.password_visible = False
        except Exception as exc:
            messagebox.showerror("Vault error", f"Account was not saved:\n{exc}", parent=self)

    def load_selected(self):
        item = self._selected()
        if not item:
            messagebox.showinfo("Select account", "Select an account from the list first.", parent=self)
            return
        self.username_entry.delete(0, "end")
        self.username_entry.insert(0, item["username"])
        self.password_entry.delete(0, "end")
        self.password_entry.insert(0, item["password"])
        self.password_entry.configure(show="")
        self.password_visible = True

    def copy_username(self):
        item = self._selected()
        if not item:
            messagebox.showinfo("Select account", "Select an account first.", parent=self)
            return
        self.clipboard_clear()
        self.clipboard_append(item["username"])
        self.log("Username copied to clipboard.")

    def copy_password(self):
        item = self._selected()
        if not item:
            messagebox.showinfo("Select account", "Select an account first.", parent=self)
            return
        self.clipboard_clear()
        self.clipboard_append(item["password"])
        self.log("Password copied to clipboard.")

    def delete_selected(self):
        item = self._selected()
        if item:
            self.delete_account(item["id"])
        else:
            messagebox.showinfo("Select account", "Select an account first.", parent=self)

    def delete_account(self, aid):
        item = next((x for x in self.accounts if x["id"] == aid), None)
        if not item:
            return
        if not messagebox.askyesno("Delete account", f"Delete saved login for {item['username']}?", parent=self):
            return
        self.accounts = [x for x in self.accounts if x["id"] != aid]
        self.revealed.discard(aid)
        if self.selected_id == aid:
            self.selected_id = None
        try:
            self._save_vault()
            self._render_accounts()
            self.log(f"Deleted saved Windscribe account: {item['username']}")
        except Exception as exc:
            messagebox.showerror("Vault error", str(exc), parent=self)

    def generate_username(self):
        value = "ws_" + "".join(secrets.choice(string.ascii_lowercase + string.digits) for _ in range(9))
        self.username_entry.delete(0, "end")
        self.username_entry.insert(0, value)
        self.log("Generated a username suggestion. You still need to register it with Windscribe.")

    def generate_password(self):
        groups = [string.ascii_lowercase, string.ascii_uppercase, string.digits, "!@#$%*-_=+?.,:"]
        chars = [secrets.choice(group) for group in groups]
        alphabet = "".join(groups)
        chars.extend(secrets.choice(alphabet) for _ in range(16))
        secrets.SystemRandom().shuffle(chars)
        self.password_entry.delete(0, "end")
        self.password_entry.insert(0, "".join(chars))
        self.password_entry.configure(show="")
        self.password_visible = True
        self.log("Generated a strong password.")

    def toggle_input(self):
        self.password_visible = not self.password_visible
        self.password_entry.configure(show="" if self.password_visible else "•")

    def log(self, message):
        if not hasattr(self, "log_box"):
            return
        self.log_box.configure(state="normal")
        self.log_box.insert("end", f"[{datetime.now():%H:%M:%S}]  {message}\n")
        self.log_box.see("end")
        self.log_box.configure(state="disabled")

    def clear_log(self):
        self.log_box.configure(state="normal")
        self.log_box.delete("1.0", "end")
        self.log_box.configure(state="disabled")

    def goto_accounts(self):
        self.area._parent_canvas.yview_moveto(0.48)

    def goto_registry(self):
        self.area._parent_canvas.yview_moveto(0.20)


if __name__ == "__main__":
    if os.name != "nt":
        raise SystemExit("Windscribe Pro is designed for Windows 10/11.")
    ctk.set_appearance_mode("Dark")
    ctk.set_default_color_theme("blue")
    WindscribeHub().mainloop()
