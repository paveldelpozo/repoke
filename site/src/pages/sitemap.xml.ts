import type { APIRoute } from "astro";
import { BASE, PAGES } from "../site.js";

export const GET: APIRoute = ({ site }) => {
  const abs = (lang: keyof typeof BASE, page: string) => site!.origin + BASE[lang] + page;
  const urls = PAGES.flatMap((page) =>
    (Object.keys(BASE) as (keyof typeof BASE)[]).map((lang) => `  <url>
    <loc>${abs(lang, page)}</loc>
    <xhtml:link rel="alternate" hreflang="en" href="${abs("en", page)}"/>
    <xhtml:link rel="alternate" hreflang="es" href="${abs("es", page)}"/>
    <xhtml:link rel="alternate" hreflang="x-default" href="${abs("en", page)}"/>
  </url>`),
  );
  const xml = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">
${urls.join("\n")}
</urlset>
`;
  return new Response(xml, { headers: { "Content-Type": "application/xml" } });
};
