(() => {
  const dialog = document.getElementById("readme-dialog");
  const trigger = document.getElementById("readme-trigger");
  const closeButton = dialog?.querySelector(".dialog-close");
  const dismissButton = dialog?.querySelector(".dialog-dismiss");

  if (!dialog || !trigger || !closeButton || !dismissButton) return;

  let lastFocused = null;

  const close = () => {
    if (typeof dialog.close === "function" && dialog.open) dialog.close();
  };

  trigger.addEventListener("click", () => {
    lastFocused = document.activeElement;
    if (typeof dialog.showModal !== "function") {
      window.location.href = "https://github.com/cybercore-tech/agentforge/blob/main/README.md";
      return;
    }
    dialog.showModal();
    closeButton.focus();
  });

  closeButton.addEventListener("click", close);
  dismissButton.addEventListener("click", close);
  dialog.addEventListener("click", (event) => {
    if (event.target === dialog) close();
  });
  dialog.addEventListener("close", () => {
    if (lastFocused && typeof lastFocused.focus === "function") lastFocused.focus();
  });
})();
