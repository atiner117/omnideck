<!-- Live wallpaper: the PSP/XMB-style wave — three translucent accent-tinted ribbons
     drifting slowly across the lower half of the screen. Deliberately cheap: a half-
     resolution canvas scaled up by CSS, capped at 30 fps, paused while the document is
     hidden, and a single static frame under prefers-reduced-motion. Sits in the
     background stack between the base color/image and the readability fade. -->
<script lang="ts">
  let { accent }: { accent: string } = $props();
  let canvas: HTMLCanvasElement | undefined = $state();

  function rgb(hex: string): [number, number, number] {
    const m = /^#([0-9a-f]{6})$/i.exec(hex);
    const n = m ? parseInt(m[1], 16) : 0x4cc2ff;
    return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  }

  $effect(() => {
    const cv = canvas;
    if (!cv) return;
    const ctx = cv.getContext("2d");
    if (!ctx) return;
    const [r, g, b] = rgb(accent);
    const still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    let raf = 0;
    let last = 0;
    const size = () => {
      // Half-resolution: the ribbons are soft gradients, upscaling is invisible and
      // quarters the pixels touched per frame (kind to the software-compositing path).
      cv.width = Math.max(320, Math.floor(window.innerWidth / 2));
      cv.height = Math.max(180, Math.floor(window.innerHeight / 2));
    };
    size();

    const draw = (t: number) => {
      const w = cv.width, h = cv.height;
      ctx.clearRect(0, 0, w, h);
      // Three ribbons back to front; each is a base sine plus a slower harmonic so the
      // crest undulates instead of scrolling rigidly.
      for (let i = 2; i >= 0; i--) {
        const yBase = h * (0.62 + 0.13 * i);
        const amp = h * (0.055 + 0.035 * i);
        const k = (Math.PI * 2) / (w * (1.15 - 0.18 * i));
        const p1 = t * (0.00012 + 0.00005 * i);
        const p2 = -t * 0.00007 + i * 2.1;
        ctx.beginPath();
        ctx.moveTo(0, h);
        for (let x = 0; x <= w; x += 8) {
          const y = yBase + amp * Math.sin(k * x + p1) + amp * 0.5 * Math.sin(k * 2.3 * x + p2);
          ctx.lineTo(x, y);
        }
        ctx.lineTo(w, h);
        ctx.closePath();
        const grad = ctx.createLinearGradient(0, yBase - amp, 0, h);
        grad.addColorStop(0, `rgba(${r},${g},${b},${0.16 - 0.04 * i})`);
        grad.addColorStop(1, `rgba(${r},${g},${b},0.01)`);
        ctx.fillStyle = grad;
        ctx.fill();
        // A faint brighter crest line sells the "ribbon" read.
        ctx.strokeStyle = `rgba(255,255,255,${0.05 - 0.012 * i})`;
        ctx.lineWidth = 1;
        ctx.stroke();
      }
    };

    if (still) {
      draw(40000); // one pleasant static frame
      const onResize = () => { size(); draw(40000); };
      window.addEventListener("resize", onResize);
      return () => window.removeEventListener("resize", onResize);
    }

    const loop = (t: number) => {
      raf = requestAnimationFrame(loop);
      if (t - last < 41) return; // ~24 fps is plenty for slow water (and kind to sw paint)
      last = t;
      draw(t);
    };
    raf = requestAnimationFrame(loop);
    const onVis = () => {
      cancelAnimationFrame(raf);
      if (!document.hidden) raf = requestAnimationFrame(loop);
    };
    const onResize = () => size();
    document.addEventListener("visibilitychange", onVis);
    window.addEventListener("resize", onResize);
    return () => {
      cancelAnimationFrame(raf);
      document.removeEventListener("visibilitychange", onVis);
      window.removeEventListener("resize", onResize);
    };
  });
</script>

<canvas bind:this={canvas} class="waves" aria-hidden="true"></canvas>

<style>
  .waves { position: fixed; inset: 0; width: 100vw; height: 100vh; pointer-events: none; }
</style>
