import numpy as np

from compose import OUT, SR, edges, filt, impact, impulse, noise, place, reverb, secs, sweep, write


def ui(x, peak_db=-6.0):
    x = x if x.ndim == 2 else place(x, 0) / np.sqrt(2)
    x = edges(x, 0.0008, 0.006)
    return x * 10 ** (peak_db / 20) / np.abs(x).max()


def sfx():
    def env(n, decay):
        return np.exp(-secs(n) / decay)
    n = int(0.06 * SR)
    t = secs(n)
    click = (np.sin(2 * np.pi * 1150 * t) * env(n, 0.005) * 0.5 + np.sin(2 * np.pi * 380 * t) * env(n, 0.012) * 0.7
             + filt(noise(n), "band", [2000, 6000]) * env(n, 0.0012) * 0.4)
    n = int(0.12 * SR)
    t = secs(n)
    f = 560 + 420 * (1 - np.exp(-t / 0.012))
    ph = 2 * np.pi * np.cumsum(f) / SR
    menu = (np.sin(ph) + 0.12 * np.sin(2 * ph)) * np.minimum(1, t / 0.0015) * env(n, 0.03)
    menu += filt(noise(n), "low", 3000) * env(n, 0.002) * 0.15
    n = int(0.05 * SR)
    t = secs(n)
    key = np.sin(2 * np.pi * 240 * t) * env(n, 0.008) * 0.7 + filt(noise(n), "band", [900, 4500]) * env(n, 0.0025) * 0.5
    late = int(0.011 * SR)
    key[late:] += filt(noise(n - late), "band", [600, 3000]) * env(n - late, 0.003) * 0.25
    n = int(0.7 * SR)
    u = secs(n) / 0.7
    body = 0.7 * noise(n)[:, None] + 0.3 * noise(n, 2)
    whoosh = sweep(body, lambda s: [c := 300 * 6 ** np.sin(np.pi * s / 0.7), c * 2.2], "band", 1)
    whoosh = filt(whoosh, "low", 4500) * (np.sin(np.pi * u ** 0.85) ** 2)[:, None]
    whoosh *= np.stack([np.cos(u * np.pi / 2), np.sin(u * np.pi / 2)], 1) * 1.2 + 0.3
    hit = impact(1.0, 1.5)
    hit = hit + filt(reverb(hit, impulse(1.0, (0.9, 0.6, 0.3), 3)), "low", 3000) * 0.25
    hit *= (np.cos(np.clip((secs(len(hit)) - 0.9) / 0.6, 0, 1) * np.pi / 2) ** 2)[:, None]
    n = int(0.09 * SR)
    t = secs(n)
    tick = (np.sin(2 * np.pi * 1568 * t) + 0.3 * np.sin(2 * np.pi * 2349 * t)) * np.minimum(1, t / 0.001) * env(n, 0.016)
    for name, x in (("click", click), ("menu", menu), ("key", key), ("whoosh", whoosh), ("impact", hit), ("tick", tick)):
        write(OUT / "sfx" / f"{name}.wav", ui(x))
