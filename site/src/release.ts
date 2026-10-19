// Gitea redirects this to the newest release.
export const RELEASES = "https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice/releases/latest";

export type Release = { version: string; downloads: number };

/** The newest release and all downloads, from the worker. */
export async function latest(): Promise<Release> {
  const res = await fetch("/release.json");
  if (!res.ok) throw new Error(`release.json: ${res.status} ${await res.text()}`);
  return (await res.json()) as Release;
}
