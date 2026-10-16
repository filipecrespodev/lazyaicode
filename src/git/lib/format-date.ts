import { intlLocale } from "../../lib/i18n/react";

// Formatters follow the interface language, cached per locale so a language
// switch takes effect on the next call.
const cache = new Map<string, { date: Intl.DateTimeFormat; dateTime: Intl.DateTimeFormat }>();
function formatters() {
  const locale = intlLocale();
  let f = cache.get(locale);
  if (!f) {
    f = {
      date: new Intl.DateTimeFormat(locale, { day: "2-digit", month: "2-digit", year: "numeric" }),
      dateTime: new Intl.DateTimeFormat(locale, {
        day: "2-digit",
        month: "2-digit",
        year: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      }),
    };
    cache.set(locale, f);
  }
  return f;
}

type DateInput = Date | number;
/** Numbers are unix seconds (what the backend sends). */
const toDate = (d: DateInput) => (typeof d === "number" ? new Date(d * 1000) : d);

/** "20/02/2023" (pt), "02/20/2023" (en)… */
export const formatDate = (d: DateInput) => formatters().date.format(toDate(d));

/** "20/02/2023 14:05" (pt), "02/20/2023 02:05 PM" (en)… */
export const formatDateTime = (d: DateInput) => formatters().dateTime.format(toDate(d)).replace(",", "");
