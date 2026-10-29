// Theme and language menus (remembered when storage is available) and copy buttons on commands.
// The saved theme is applied, and the language chosen, by the inline script in <head>.
(function () {
  var root = document.documentElement;

  function save(key, value) {
    try { localStorage.setItem(key, value); } catch (e) {}
  }

  document.addEventListener("DOMContentLoaded", function () {
    var theme = document.getElementById("theme");
    if (theme) {
      theme.value = root.dataset.theme || "system";
      theme.addEventListener("change", function () {
        if (theme.value === "system") {
          delete root.dataset.theme;
        } else {
          root.dataset.theme = theme.value;
        }
        save("theme", theme.value);
      });
    }

    var lang = document.getElementById("lang");
    if (lang) {
      lang.addEventListener("change", function () {
        save("lang", lang.options[lang.selectedIndex].lang);
        location.href = lang.value + location.hash;
      });
    }

    // Install tabs: the panel shown is the one named by <html data-os>, set in <head>.
    var tabs = Array.prototype.slice.call(document.querySelectorAll('[role="tab"][data-os]'));
    function select(tab, focus) {
      root.dataset.os = tab.dataset.os;
      tabs.forEach(function (t) {
        t.setAttribute("aria-selected", t === tab ? "true" : "false");
        t.tabIndex = t === tab ? 0 : -1;
      });
      if (focus) tab.focus();
    }
    tabs.forEach(function (tab, i) {
      if (tab.dataset.os === root.dataset.yourOs) {
        var hint = document.createElement("span");
        hint.className = "yours";
        hint.textContent = tab.closest("[data-yours]").dataset.yours;
        tab.appendChild(hint);
      }
      tab.addEventListener("click", function () { select(tab); });
      tab.addEventListener("keydown", function (e) {
        var step = { ArrowRight: 1, ArrowLeft: -1 }[e.key];
        if (e.key === "Home" || e.key === "End") step = 0;
        if (step === undefined) return;
        e.preventDefault();
        var next = e.key === "Home" ? 0 : e.key === "End" ? tabs.length - 1 : (i + step + tabs.length) % tabs.length;
        select(tabs[next], true);
      });
      if (tab.dataset.os === root.dataset.os) select(tab);
    });

    // Command blocks get a copy button; real output (pre.output) doesn't.
    if (!navigator.clipboard) return;
    var text = document.body.dataset;
    document.querySelectorAll("pre:not(.output)").forEach(function (pre) {
      var copy = document.createElement("button");
      copy.type = "button";
      copy.className = "copy";
      copy.textContent = text.copy;
      copy.setAttribute("aria-live", "polite");
      copy.addEventListener("click", function () {
        // The commands without their trailing `# comments`.
        var commands = (pre.querySelector("code") || pre).innerText
          .split("\n")
          .map(function (line) { return line.replace(/\s+#\s.*$/, ""); })
          .join("\n")
          .trim();
        navigator.clipboard.writeText(commands).then(function () {
          copy.textContent = text.copied;
          copy.dataset.done = "";
          setTimeout(function () {
            copy.textContent = text.copy;
            delete copy.dataset.done;
          }, 2000);
        }, function () {
          copy.textContent = text.copyFailed;
        });
      });
      var wrap = document.createElement("div");
      wrap.className = "code";
      pre.parentNode.insertBefore(wrap, pre);
      wrap.appendChild(pre);
      wrap.appendChild(copy);
    });
  });
})();
