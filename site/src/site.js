import cargo from "../../Cargo.toml?raw";

// Cargo.toml changes only in a release commit, so its version is the latest one published.
export const version = cargo.match(/^version = "(.+)"$/m)[1];

export const BASE = { en: "/", es: "/es/" };
export const OG = { en: ["og.png", "en_US"], es: ["og-es.png", "es_ES"] };
