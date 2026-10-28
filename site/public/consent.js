// Google Analytics only loads after the visitor accepts it (RGPD / LSSI).
// Same file in paveldelpozo-web/public/consent.js and hivefold apps/landing/src/scripts/consent.js.
(function () {
  var KEY = "analytics-consent";
  var banner = document.getElementById("consent");
  if (!banner) return;
  var id = banner.dataset.gaId;

  function stored() {
    try { return localStorage.getItem(KEY); } catch (e) { return null; }
  }

  function load() {
    if (window.gtag) return;
    window.dataLayer = window.dataLayer || [];
    window.gtag = function () { window.dataLayer.push(arguments); };
    window.gtag("js", new Date());
    window.gtag("config", id);
    var script = document.createElement("script");
    script.async = true;
    script.src = "https://www.googletagmanager.com/gtag/js?id=" + id;
    document.head.appendChild(script);
  }

  // GA writes its cookies on the registrable domain, shared by every *.paveldelpozo.es site.
  function clearCookies() {
    var domain = "." + location.hostname.split(".").slice(-2).join(".");
    document.cookie.split(";").forEach(function (c) {
      var name = c.split("=")[0].trim();
      if (name.indexOf("_ga") === 0) document.cookie = name + "=; Max-Age=0; path=/; domain=" + domain;
    });
  }

  function choose(value) {
    var wasLoaded = !!window.gtag;
    try { localStorage.setItem(KEY, value); } catch (e) {}
    banner.hidden = true;
    if (value === "granted") return load();
    clearCookies();
    if (wasLoaded) location.reload();
  }

  banner.querySelectorAll("[data-consent]").forEach(function (button) {
    button.addEventListener("click", function () { choose(button.dataset.consent); });
  });
  document.querySelectorAll("[data-consent-open]").forEach(function (button) {
    button.addEventListener("click", function () {
      banner.hidden = false;
      banner.querySelector("[data-consent]").focus();
    });
  });

  var choice = stored();
  if (choice === "granted") load();
  else if (choice !== "denied") banner.hidden = false;
})();
