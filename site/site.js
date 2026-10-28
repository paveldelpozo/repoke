// Theme switch (remembered when storage is available) and copy buttons on command blocks.
(function () {
  var root = document.documentElement;

  function stored() {
    try { return localStorage.getItem("theme"); } catch (e) { return null; }
  }

  function current() {
    return root.dataset.theme ||
      (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light");
  }

  function label(button) {
    var dark = current() === "dark";
    button.textContent = dark ? "Light" : "Dark";
    button.setAttribute("aria-pressed", String(dark));
    button.setAttribute("aria-label", dark ? "Switch to the light theme" : "Switch to the dark theme");
  }

  if (stored()) root.dataset.theme = stored();

  document.addEventListener("DOMContentLoaded", function () {
    var button = document.querySelector(".theme");
    if (button) {
      label(button);
      button.addEventListener("click", function () {
        root.dataset.theme = current() === "dark" ? "light" : "dark";
        try { localStorage.setItem("theme", root.dataset.theme); } catch (e) {}
        label(button);
      });
    }

    // Command blocks get a copy button; real output (pre.output) doesn't.
    if (!navigator.clipboard) return;
    document.querySelectorAll("pre:not(.output)").forEach(function (pre) {
      var copy = document.createElement("button");
      copy.type = "button";
      copy.className = "copy";
      copy.textContent = "Copy";
      copy.setAttribute("aria-live", "polite");
      copy.addEventListener("click", function () {
        // Copy the commands without their trailing `# comments`.
        var text = (pre.querySelector("code") || pre).innerText
          .split("\n")
          .map(function (line) { return line.replace(/\s+#\s.*$/, ""); })
          .join("\n")
          .trim();
        navigator.clipboard.writeText(text).then(function () {
          copy.textContent = "Copied";
          copy.dataset.done = "";
          setTimeout(function () {
            copy.textContent = "Copy";
            delete copy.dataset.done;
          }, 2000);
        }, function () {
          copy.textContent = "Select and copy";
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
