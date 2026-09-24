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

(() => {
  const REPO = "https://github.com/cybercore-tech/agentforge";
  const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
  const grid = document.getElementById("updates-grid");
  const summary = document.getElementById("updates-summary");
  if (!grid || !summary || typeof fetch !== "function") return;

  const element = (tag, className, text) => {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
  };

  // Records use Markdown backticks for code; render them as <code> without ever parsing HTML.
  const richText = (tag, className, text) => {
    const node = element(tag, className);
    String(text).split(/`([^`]+)`/).forEach((part, index) => {
      node.append(index % 2 ? element("code", "", part) : document.createTextNode(part));
    });
    return node;
  };

  const formatDate = (value) => {
    const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value || "");
    return match ? `${MONTHS[Number(match[2]) - 1]} ${Number(match[3])}, ${match[1]}` : "";
  };

  const milestoneItem = (milestone) => {
    const item = element("li", "update-item");
    const meta = element("div", "update-meta");
    meta.append(element("span", "update-id", milestone.id));
    if (milestone.completed) {
      const time = element("time", "", formatDate(milestone.completed));
      time.dateTime = milestone.completed;
      meta.append(time);
    } else if (milestone.status === "active") {
      meta.append(element("span", "update-live", "in progress"));
    }
    item.append(meta, richText("h4", "", milestone.title), richText("p", "", milestone.signal));
    if (milestone.plan) {
      const link = element("a", "update-link", "Plan and evidence");
      link.href = `${REPO}/blob/main/${milestone.plan}`;
      link.append(element("span", "", " →"));
      link.lastChild.setAttribute("aria-hidden", "true");
      item.append(link);
    }
    return item;
  };

  const render = (feed) => {
    const recent = document.getElementById("updates-recent");
    feed.recent.forEach((milestone) => recent.append(milestoneItem(milestone)));

    if (feed.in_progress.length) {
      const active = document.getElementById("updates-active");
      feed.in_progress.forEach((milestone) => active.append(milestoneItem(milestone)));
      document.getElementById("updates-active-wrap").hidden = false;
    }

    const phases = document.getElementById("updates-phases");
    feed.phases.forEach((phase) => {
      const item = element("li", "phase-item");
      const label = element("div", "phase-label");
      label.append(element("span", "", `Phase ${phase.number} · ${phase.name}`));
      label.append(element("b", "", `${phase.complete}/${phase.total}`));
      const bar = element("div", "phase-bar");
      bar.setAttribute("role", "progressbar");
      bar.setAttribute("aria-label", `Phase ${phase.number} progress`);
      bar.setAttribute("aria-valuemin", "0");
      bar.setAttribute("aria-valuemax", String(phase.total));
      bar.setAttribute("aria-valuenow", String(phase.complete));
      const fill = element("span");
      fill.style.width = `${phase.total ? (100 * phase.complete) / phase.total : 0}%`;
      bar.append(fill);
      item.append(label, bar);
      phases.append(item);
    });

    const unreleased = document.getElementById("updates-unreleased");
    Object.entries(feed.unreleased).forEach(([group, entries]) => {
      if (!entries.length) return;
      unreleased.append(element("h4", `unreleased-group group-${group.toLowerCase()}`, group));
      const list = element("ul", "unreleased-list");
      entries.forEach((entry) => list.append(richText("li", "", entry)));
      unreleased.append(list);
    });

    summary.textContent = `${feed.totals.complete} of ${feed.totals.total} planned milestones shipped. Generated from the repository's milestone records, plans, and changelog when the site was published.`;
    grid.hidden = false;
  };

  fetch("updates.json", { cache: "no-cache" })
    .then((response) => {
      if (!response.ok) throw new Error(`updates feed returned ${response.status}`);
      return response.json();
    })
    .then((feed) => {
      if (feed.version !== 1) throw new Error("unsupported updates feed version");
      render(feed);
    })
    .catch(() => {
      summary.textContent = "The live milestone feed is not available here. Follow the links below for the full history.";
    });
})();
