import "./style.css";
import "./iso.css";
import "./tour.css";
import { tour } from "./tour.ts";
import { props, reveal } from "./regions.ts";
import { night } from "./night.ts";
import { rhythm } from "./rhythm.ts";
import { playground } from "./play.ts";
import { film } from "./film.ts";
import { said, speak, word, type Lang } from "./text.ts";
import { RELEASES, latest } from "./release.ts";
import { iconLink } from "./icons.ts";

const REPO = "https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice";

const app = document.querySelector<HTMLElement>("#app");
if (!app) throw new Error("index.html has no #app");

const hero = document.createElement("header");
const title = document.createElement("h1");
title.textContent = "Ely Office";
const source = iconLink("gitea", "source", REPO);
const download = iconLink("download", "get", RELEASES);
const links = document.createElement("div");
links.className = "links";
links.append(download, source);
const heroCount = said("p", "count");
heroCount.hidden = true;
hero.append(title, said("p", "tagline"), said("p", "platforms"), heroCount, links);
const { section, host, svg } = tour(hero);
rhythm(host, svg);

const shelf = props();

const footer = document.createElement("footer");
footer.id = "download";
const version = document.createElement("h2");
version.textContent = "Ely Office";
const count = said("p", "count");
count.hidden = true;
const get = said("a", "get");
get.href = RELEASES;
footer.append(version, count, said("p", "platforms"), said("p", "needs"), said("p", "unsigned"), get);

const toggle = said("button", "other");
toggle.className = "lang";
let lang: Lang = navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en";
/** Writes the page in `next`; the toggle names the other. */
const say = (next: Lang) => {
  lang = next;
  speak(lang);
  toggle.lang = lang === "en" ? "zh-CN" : "en";
};
toggle.addEventListener("click", () => say(lang === "en" ? "zh" : "en"));

const main = document.createElement("main");
main.append(section, playground(), shelf, film());
app.append(night(), toggle, main, footer);
say(lang);
reveal([shelf]);
void latest().then(({ version: v, downloads }) => {
  version.textContent = `Ely Office ${v}`;
  for (const el of [heroCount, count]) {
    el.dataset.n = String(downloads);
    el.textContent = word(lang, "count", downloads);
    el.hidden = false;
  }
});
