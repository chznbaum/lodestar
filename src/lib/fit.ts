export type FitBandKey = "strong" | "good" | "partial" | "weak" | "mismatch" | "unscored";

/**
 * Map an integer fit score (0–100, or null for unscored) to a band key and human label.
 * Bands: >=80 strong · >=60 good · >=40 partial · >=20 weak · >=0 mismatch · null unscored.
 */
export function fitBand(score: number | null): { key: FitBandKey; label: string } {
  if (score === null) return { key: "unscored", label: "Unscored" };
  if (score >= 80) return { key: "strong", label: "Strong" };
  if (score >= 60) return { key: "good", label: "Good" };
  if (score >= 40) return { key: "partial", label: "Partial" };
  if (score >= 20) return { key: "weak", label: "Weak" };
  return { key: "mismatch", label: "Mismatch" };
}
