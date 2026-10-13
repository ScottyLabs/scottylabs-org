import useEmblaCarousel from "embla-carousel-react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { useCallback, useSyncExternalStore } from "react";
import type { CarouselPhoto } from "../../data/history";
import css from "./carousel.module.css";

const HISTORY_ASSETS_BASE_URL = "https://assets.scottylabs.org/team-page/history/";

export const HistoryCarousel: React.FC<{ photos: CarouselPhoto[] }> = ({ photos }) => {
  const [emblaRef, emblaApi] = useEmblaCarousel({
    align: "start",
    containScroll: "trimSnaps",
    dragFree: true,
  });

  const subscribe = useCallback(
    (onChange: () => void) => {
      emblaApi?.on("select", onChange).on("reInit", onChange);
      return () => {
        emblaApi?.off("select", onChange).off("reInit", onChange);
      };
    },
    [emblaApi],
  );
  const canPrev = useSyncExternalStore(
    subscribe,
    () => emblaApi?.canScrollPrev() ?? false,
    () => false,
  );
  const canNext = useSyncExternalStore(
    subscribe,
    () => emblaApi?.canScrollNext() ?? false,
    () => false,
  );

  const scrollPrev = useCallback(() => {
    emblaApi?.scrollPrev();
  }, [emblaApi]);
  const scrollNext = useCallback(() => {
    emblaApi?.scrollNext();
  }, [emblaApi]);

  return (
    <div className={css.root}>
      <div className={css.viewport} ref={emblaRef}>
        <div className={css.container}>
          {photos.map((photo) => (
            <div key={photo.filename} className={css.slide}>
              <img
                src={`${HISTORY_ASSETS_BASE_URL}${photo.filename}`}
                alt={photo.caption ?? ""}
                className={css.image}
                loading="lazy"
                decoding="async"
              />
              {photo.caption === undefined ? null : (
                <div className={css.caption}>{photo.caption}</div>
              )}
            </div>
          ))}
        </div>
      </div>
      <button
        type="button"
        className={`${css.button} ${css.prev}`}
        onClick={scrollPrev}
        disabled={!canPrev}
        aria-label="Previous"
      >
        <ChevronLeft size={20} />
      </button>
      <button
        type="button"
        className={`${css.button} ${css.next}`}
        onClick={scrollNext}
        disabled={!canNext}
        aria-label="Next"
      >
        <ChevronRight size={20} />
      </button>
    </div>
  );
};
