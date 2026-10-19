// Answers /release.json with the newest release and downloads.
const REPO = "https://git.zacharyzhang.com/api/v1/repos/ZacharyZhang-NY/ElyOffice";
const PAGE = 50;

type Cached = RequestInit & { cf: { cacheTtl: number; cacheEverything: boolean } };
type Asset = { name: string; download_count: number };
type Release = { tag_name: string; assets: Asset[] };

const init: Cached = {
  headers: { accept: "application/json", "user-agent": "ely-office-site" },
  cf: { cacheTtl: 300, cacheEverything: true },
};

async function read<T>(path: string): Promise<T> {
  const res = await fetch(`${REPO}${path}`, init);
  if (!res.ok) throw new Error(`Gitea answered ${res.status} for ${path}`);
  return (await res.json()) as T;
}

/** The newest release's version, without the v. */
async function version(): Promise<string> {
  const { tag_name: tag } = await read<Release>("/releases/latest");
  if (!/^v\d+\.\d+\.\d+$/.test(tag)) throw new Error(`Gitea's latest tag is ${JSON.stringify(tag)}`);
  return tag.slice(1);
}

/** Downloads of every release; update manifests are checks, not downloads. */
async function downloads(): Promise<number> {
  let total = 0;
  for (let page = 1; ; page++) {
    const releases = await read<Release[]>(`/releases?limit=${PAGE}&page=${page}`);
    for (const { assets } of releases) {
      for (const a of assets) {
        if (!Number.isInteger(a.download_count)) throw new Error(`${a.name} has no download count`);
        if (!a.name.endsWith(".json")) total += a.download_count;
      }
    }
    if (releases.length < PAGE) return total;
  }
}

export default {
  async fetch(): Promise<Response> {
    try {
      const [v, n] = await Promise.all([version(), downloads()]);
      return Response.json({ version: v, downloads: n }, { headers: { "cache-control": "public, max-age=300" } });
    } catch (e) {
      return new Response(String(e), { status: 502 });
    }
  },
};
