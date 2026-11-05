import cargo from "../../Cargo.toml?raw";

// Cargo.toml changes only in a release commit, so its version is the latest one published.
export const version = cargo.match(/^version = "(.+)"$/m)[1];

export const BASE = { en: "/", es: "/es/" };
export const OG = { en: ["og.png", "en_US"], es: ["og-es.png", "es_ES"] };

// The page background in each color scheme, from style.css.
export const THEME_COLOR = { light: "#FBF9F4", dark: "#0F2A22" };

// Page files under each language base; the sitemap is built from these.
export const PAGES = ["", "commands.html", "plugins.html"];
