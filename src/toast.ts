export function showToast(message: string, ms = 3200): void {
  let host = document.getElementById("toast-host");
  if (!host) {
    host = document.createElement("div");
    host.id = "toast-host";
    document.body.appendChild(host);
  }
  const el = document.createElement("div");
  el.textContent = message;
  Object.assign(el.style, {
    pointerEvents: "auto",
    background: "#323232",
    color: "#fff",
    padding: "12px 20px",
    borderRadius: "8px",
    fontSize: "14px",
    boxShadow: "0 4px 12px rgba(0,0,0,0.2)",
    maxWidth: "420px",
    textAlign: "center",
  } as CSSStyleDeclaration);
  host.appendChild(el);
  setTimeout(() => {
    el.style.opacity = "0";
    el.style.transition = "opacity 200ms";
    setTimeout(() => el.remove(), 220);
  }, ms);
}
