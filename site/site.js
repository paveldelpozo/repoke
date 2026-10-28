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
