import { useEffect, useMemo, useRef, useState } from "react";
import css from "./ImageCarousel.module.css";
import clsx from "clsx";

function shuffleArray(ar: string[]) {
  const shuffledAr = [...ar];
  const N = shuffledAr.length;
  for (let i = 0; i < N; i++) {
    // rand([i,N-1])
    const randI = Math.floor(Math.random() * (N - i)) + i;
    [shuffledAr[i], shuffledAr[randI]] = [shuffledAr[randI], shuffledAr[i]];
  }
  return shuffledAr;
}
export default function ImageCarousel({
  imageLinks,
  speedPxPerSecond,
  heightPx,
  gapPx,
  PeriodicTileInsert,
}: {
  imageLinks: string[];
  speedPxPerSecond: number;
  heightPx: number;
  gapPx?: number;
  PeriodicTileInsert?: () => React.ReactNode;
}) {
  const carouselRef = useRef<HTMLDivElement | null>(null);
  const endOfCarouselRef = useRef<HTMLDivElement | null>(null);
  const direction = speedPxPerSecond > 0 ? "right" : "left";
  const LOADING_BUFFER_PX = 500;
  const [imagesVisibleOnScreen, setImagesVisibleOnScreen] = useState(5);
  const shuffledImageLinks = useMemo(() => shuffleArray(imageLinks), [imageLinks]);
  const INITIAL_IMAGES_TO_LOAD = 10;

  useEffect(() => {
    const updateCustomTilePositioning = () => {
      setImagesVisibleOnScreen(Math.max(2, Math.ceil(window.innerWidth / 300)));
    };
    window.addEventListener("resize", updateCustomTilePositioning);
    updateCustomTilePositioning();
    return () => {
      window.removeEventListener("resize", updateCustomTilePositioning);
    };
  }, []);
  const [tiles, setTiles] = useState<React.ReactNode[]>([]);

  useEffect(() => {
    // tiles come from a client-side shuffle, so they can't be in the prerendered HTML
    // oxlint-disable-next-line react/set-state-in-effect
    setTiles(
      Array.from({ length: INITIAL_IMAGES_TO_LOAD }, (_, i) => (
        <img
          className={css["carousel__image"]}
          src={shuffledImageLinks[i % shuffledImageLinks.length]}
          alt=""
          key={Math.random()}
          // the key doesn't matter since we're never changing this element
        />
      )),
    );
    let prevRenderTime = performance.now();
    let totalElapsed = 0;
    let running = true;

    const updatePosition = (curRenderTime: number) => {
      if (!running) return;
      // cap the step so returning to a backgrounded tab doesn't trigger a burst of image loads
      totalElapsed += Math.min(500, curRenderTime - prevRenderTime);
      prevRenderTime = curRenderTime;
      const offsetX = (totalElapsed / 1000) * speedPxPerSecond;
      if (carouselRef.current && endOfCarouselRef.current) {
        const { left: posX } = endOfCarouselRef.current.getBoundingClientRect();

        if (
          (direction === "left" && posX <= window.innerWidth + LOADING_BUFFER_PX) ||
          (direction === "right" && posX >= -LOADING_BUFFER_PX && totalElapsed >= 1000)
        ) {
          // innerWidth accounts for page zoom
          // add in an additional image (or manual tile)

          setTiles((prev) => [
            ...prev,
            <img
              className={css["carousel__image"]}
              // breaks if tiles leaving the viewport are ever removed
              src={shuffledImageLinks[prev.length % shuffledImageLinks.length]}
              alt=""
              key={Math.random()}
              // the key doesn't matter since we're never changing this element
            />,
          ]);
        }
        carouselRef.current.style.translate = `${offsetX}px`;
      }
      requestAnimationFrame(updatePosition);
    };
    requestAnimationFrame(updatePosition);
    return () => {
      running = false;
    };
  }, [speedPxPerSecond, shuffledImageLinks, LOADING_BUFFER_PX, direction]);

  return (
    <div
      className={clsx(
        css["carousel-container"],
        direction === "right" && css["carousel-container--flipped"],
      )}
    >
      <div className={css["carousel"]} ref={carouselRef} style={{ height: heightPx, gap: gapPx }}>
        {tiles.flatMap((tile, i) => {
          if ((i + 2) % (imagesVisibleOnScreen - 1) === 0 && PeriodicTileInsert) {
            // insert tile
            return [
              tile,
              <div key={Math.floor(i / (imagesVisibleOnScreen - 1))} style={{ flexShrink: 0 }}>
                <PeriodicTileInsert />
              </div>,
            ];
          }
          return [tile];
        })}
        <div ref={endOfCarouselRef} />
      </div>
    </div>
  );
}
