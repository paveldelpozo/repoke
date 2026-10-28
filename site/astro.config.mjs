import { satteri } from "@astrojs/markdown-satteri";
import { defineConfig } from "astro/config";

const REPO = "https://github.com/paveldelpozo/repoke/blob/main/";

// Links in docs/ point into the repository (`../examples/plugins`); on the site they go to GitHub.
const repositoryLinks = {
  name: "repository-links",
  link(node, ctx) {
    if (node.url.startsWith("../")) {
      ctx.setProperty(node, "url", REPO + node.url.replace(/^(\.\.\/)+/, ""));
    }
  },
};

export default defineConfig({
  site: "https://repoke.paveldelpozo.es",
  // The pages keep the URLs they had before Astro: /commands.html, /es/index.html…
  build: { format: "preserve" },
  markdown: {
    // The docs are rendered as written: no colored code and no curly quotes or dashes in `--flags`.
    syntaxHighlight: false,
    processor: satteri({
      features: { smartPunctuation: false },
      mdastPlugins: [repositoryLinks],
    }),
  },
});
