import { fetchApi } from "@/lib/api-client";
import type {
  BackfillHistoricalResult,
  CacheScope,
  CalibrationReport,
  InvalidateCacheResult,
  PrewarmResult,
  ProcessRankingsResult,
  RebackfillCarouselResult,
  RehydrateResult,
  SweepParams,
  SweepReport,
} from "./types";

// All admin endpoints are GET-only today. Each helper returns the
// `data` field of the standard API envelope; errors bubble up via
// `fetchApi` throwing, which React Query surfaces as `error` on the
// mutation.

export const adminApi = {
  invalidateCache(scope: CacheScope) {
    return fetchApi<InvalidateCacheResult>(
      `admin/cache/invalidate?scope=${encodeURIComponent(scope)}`,
    );
  },

  processRankings(date: string) {
    return fetchApi<ProcessRankingsResult>(
      `admin/process-rankings/${encodeURIComponent(date)}`,
    );
  },

  prewarm() {
    return fetchApi<PrewarmResult>("admin/prewarm");
  },

  rehydrate() {
    return fetchApi<RehydrateResult>("admin/rehydrate");
  },

  backfillHistorical(start: string, end: string) {
    return fetchApi<BackfillHistoricalResult>(
      `admin/backfill-historical?start=${encodeURIComponent(start)}&end=${encodeURIComponent(end)}`,
    );
  },

  rebackfillCarousel(season: string) {
    return fetchApi<RebackfillCarouselResult>(
      `admin/rebackfill-carousel?season=${encodeURIComponent(season)}`,
    );
  },

  calibrate(season: string) {
    return fetchApi<CalibrationReport>(
      `admin/calibrate?season=${encodeURIComponent(season)}`,
    );
  },

  calibrateSweep(season: string, params: SweepParams) {
    const qs = new URLSearchParams({ season });
    for (const [k, v] of Object.entries(params)) {
      if (v && v.trim().length > 0) qs.set(k, v.trim());
    }
    return fetchApi<SweepReport>(`admin/calibrate-sweep?${qs.toString()}`);
  },
};
