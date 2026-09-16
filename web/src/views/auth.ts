import type { SavedAccount } from "../types";
import {
  state,
  getSavedAccounts,
  saveAccount,
  forgetAccount,
  setActiveAccount,
  clearActiveAccount,
} from "../state";
import { $, h, avatar, avatarUrl, toast } from "../utils/dom";
import { NAME_RE } from "../utils/format";
import { apiLogin, apiRegister, apiVerify } from "../api";

let connectCallback: ((name: string, token: string) => void) | null = null;

export function setConnectCallback(cb: (name: string, token: string) => void): void {
  connectCallback = cb;
}

export function showLogin(error?: string): void {
  state.me = null;
  state.token = null;
  state.ws?.close();
  state.ws = null;
  $("app")?.classList.add("hidden");
  $("login")?.classList.remove("hidden");
  const errEl = $("login-error");
  if (errEl) {
    errEl.textContent = error ?? "";
    errEl.classList.toggle("hidden", !error);
  }
  renderSavedAccounts();
  updateLoginAvatar();
}

export function showApp(): void {
  $("login")?.classList.add("hidden");
  $("app")?.classList.remove("hidden");
}

export function updateLoginAvatar(): void {
  const input = $("login-name") as HTMLInputElement | null;
  const img = $("login-avatar") as HTMLImageElement | null;
  if (img) {
    img.src = avatarUrl(input?.value.trim() || "forest");
  }
}

export function showAuthForm(): void {
  $("saved-accounts-box")?.classList.add("hidden");
  $("auth-box")?.classList.remove("hidden");
  ($("login-name") as HTMLInputElement | null)?.focus();
}

export function renderSavedAccounts(): void {
  const accounts = getSavedAccounts();
  const box = $("saved-accounts-box");
  const list = $("saved-accounts-list");
  const authBox = $("auth-box");
  const backWrap = $("btn-back-saved-wrap");

  if (!box || !list || !authBox || !backWrap) return;

  if (accounts.length === 0) {
    box.classList.add("hidden");
    authBox.classList.remove("hidden");
    backWrap.classList.add("hidden");
    return;
  }

  box.classList.remove("hidden");
  authBox.classList.add("hidden");
  backWrap.classList.remove("hidden");

  const nodes = accounts.map((acc) => {
    return h(
      "div",
      {
        class:
          "flex items-center gap-2 p-2 bg-base-200 rounded-lg hover:bg-base-300 transition-colors",
      },
      avatar(avatarUrl(acc.name), "w-9"),
      h(
        "div",
        {
          class: "flex-1 min-w-0 cursor-pointer",
          onclick: () => loginWithSavedAccount(acc),
        },
        h("div", { class: "font-semibold text-xs truncate" }, `@${acc.name}`),
        h("div", { class: "text-[11px] text-base-content/60" }, "Click to sign in")
      ),
      h(
        "button",
        {
          type: "button",
          class: "btn btn-xs btn-primary",
          onclick: () => loginWithSavedAccount(acc),
        },
        "Sign In"
      ),
      h(
        "button",
        {
          type: "button",
          class: "btn btn-xs btn-ghost btn-circle text-base-content/50 hover:text-error",
          title: "Remove saved account",
          onclick: (e: MouseEvent) => {
            e.stopPropagation();
            forgetAccount(acc.name);
            renderSavedAccounts();
          },
        },
        "✕"
      )
    );
  });

  list.replaceChildren(...nodes);
}

export async function loginWithSavedAccount(acc: SavedAccount): Promise<void> {
  const statusEl = $("status");
  if (statusEl) {
    statusEl.textContent = "verifying…";
    statusEl.className = "badge badge-ghost badge-sm";
  }

  try {
    const data = await apiVerify(acc.name, acc.token);
    if (data.valid) {
      saveAccount(acc.name, acc.token);
      setActiveAccount(acc.name, acc.token);
      if (connectCallback) connectCallback(acc.name, acc.token);
    } else {
      forgetAccount(acc.name);
      showAuthForm();
      const input = $("login-name") as HTMLInputElement | null;
      if (input) input.value = acc.name;
      updateLoginAvatar();
      showLogin("Session expired. Please enter your password.");
    }
  } catch (err) {
    toast(`Connection error: ${err}`, "error");
  }
}

export function initAuthUI(): void {
  $("tab-auth-login")?.addEventListener("click", () => {
    state.authMode = "login";
    const tabLogin = $("tab-auth-login");
    const tabReg = $("tab-auth-register");
    const submitBtn = $("btn-auth-submit");
    if (tabLogin) tabLogin.className = "tab tab-active font-semibold text-xs";
    if (tabReg) tabReg.className = "tab font-semibold text-xs";
    if (submitBtn) submitBtn.textContent = "Sign In";
    $("login-error")?.classList.add("hidden");
  });

  $("tab-auth-register")?.addEventListener("click", () => {
    state.authMode = "register";
    const tabReg = $("tab-auth-register");
    const tabLogin = $("tab-auth-login");
    const submitBtn = $("btn-auth-submit");
    if (tabReg) tabReg.className = "tab tab-active font-semibold text-xs";
    if (tabLogin) tabLogin.className = "tab font-semibold text-xs";
    if (submitBtn) submitBtn.textContent = "Create Account";
    $("login-error")?.classList.add("hidden");
  });

  $("btn-show-auth-form")?.addEventListener("click", showAuthForm);
  $("btn-back-saved")?.addEventListener("click", renderSavedAccounts);

  $("btn-toggle-pwd")?.addEventListener("click", () => {
    const input = $("login-password") as HTMLInputElement | null;
    if (input) input.type = input.type === "password" ? "text" : "password";
  });

  $("login-name")?.addEventListener("input", updateLoginAvatar);

  $("login-form")?.addEventListener("submit", async (e) => {
    e.preventDefault();
    const nameInput = $("login-name") as HTMLInputElement | null;
    const pwdInput = $("login-password") as HTMLInputElement | null;
    const remInput = $("login-remember") as HTMLInputElement | null;
    const submitBtn = $("btn-auth-submit") as HTMLButtonElement | null;
    const errEl = $("login-error");

    const name = (nameInput?.value || "").trim();
    const password = pwdInput?.value || "";
    const remember = Boolean(remInput?.checked);

    if (!NAME_RE.test(name)) {
      if (errEl) {
        errEl.textContent = "Use 1–24 letters, numbers, - or _.";
        errEl.classList.remove("hidden");
      }
      return;
    }
    if (password.length < 3) {
      if (errEl) {
        errEl.textContent = "Password must be at least 3 characters.";
        errEl.classList.remove("hidden");
      }
      return;
    }

    if (submitBtn) submitBtn.disabled = true;
    errEl?.classList.add("hidden");

    try {
      const data =
        state.authMode === "register"
          ? await apiRegister(name, password)
          : await apiLogin(name, password);

      if (submitBtn) submitBtn.disabled = false;

      if (data.error || !data.token || !data.name) {
        if (errEl) {
          errEl.textContent = data.error || "Authentication failed.";
          errEl.classList.remove("hidden");
        }
        return;
      }

      if (remember) {
        saveAccount(data.name, data.token);
        setActiveAccount(data.name, data.token);
      } else {
        clearActiveAccount();
      }

      state.active = null;
      state.chats.clear();
      state.dmPeers.clear();
      if (pwdInput) pwdInput.value = "";
      if (connectCallback) connectCallback(data.name, data.token);
    } catch (err) {
      if (submitBtn) submitBtn.disabled = false;
      if (errEl) {
        errEl.textContent = `Server error: ${err}`;
        errEl.classList.remove("hidden");
      }
    }
  });

  $("logout")?.addEventListener("click", () => {
    clearActiveAccount();
    showLogin();
  });
}
