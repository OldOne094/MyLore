/* MISSION-033 — Application strings. Key tree is the contract: en and ar must
   stay in sync (verified by i18n.interop test). Namespaces: "shell", "nav",
   "page". Plural-aware via Intl rules and {{count}} interpolation. */

const en = {
  shell: {
    brand: "MyLore",
    status: {
      version: "v0.1.0",
      counts_one: "{{count}} title",
      counts_other: "{{count}} titles",
    },
  },
  theme: {
    light: "Light",
    dark: "Dark",
    system: "System",
  },
  nav: {
    library: "Library",
    search: "Search",
    discover: "Discover",
    collections: "Collections",
    reviews: "Reviews",
    stats: "Stats",
    calendar: "Calendar",
    settings: "Settings",
    hint_library: "Your tracked titles appear here as you add them.",
    hint_search: "Find titles in your library or add new ones.",
    hint_discover: "Explore seasonal charts and recommendations.",
    hint_collections: "Group titles into smart and manual collections.",
    hint_reviews: "Write and manage your reviews here.",
    hint_stats: "Time watched, pages read and your ratings distribution.",
    hint_calendar: "Airing schedules and upcoming release dates.",
    hint_settings: "Theme, language, data and provider preferences.",
  },
  page: {
    status_bar: "Status bar",
  },
  settings: {
    theme: "Theme",
    themeHint: "Light, dark, or follow the system appearance.",
    language: "Language",
    languageHint: "Interface language. Arabic switches the layout to RTL.",
  },
} as const;

/* Arabic tree. Not annotated against `typeof en` because Arabic carries extra
   ICU plural categories (zero/two/few/many); key parity is enforced by the
   locales.test.ts suite instead. */

const ar = {
  shell: {
    brand: "ماي‌لور",
    status: {
      version: "v0.1.0",
      counts_zero: "{{count}} عنوان",
      counts_one: "{{count}} عنوان",
      counts_two: "{{count}} عنوانان",
      counts_few: "{{count}} عناوين",
      counts_many: "{{count}} عنواناً",
      counts_other: "{{count}} عنوان",
    },
  },
  theme: {
    light: "فاتح",
    dark: "داكن",
    system: "النظام",
  },
  nav: {
    library: "المكتبة",
    search: "البحث",
    discover: "اكتشف",
    collections: "المجموعات",
    reviews: "المراجعات",
    stats: "الإحصاءات",
    calendar: "التقويم",
    settings: "الإعدادات",
    hint_library: "ستظهر عناوينك المُتتبَّعة هنا عند إضافتها.",
    hint_search: "ابحث في مكتبتك أو أضف عناوين جديدة.",
    hint_discover: "استكشف الجداول الموسمية والتوصيات.",
    hint_collections: "جمِّع العناوين في مجموعات ذكية أو يدوية.",
    hint_reviews: "اكتب مراجعاتك وأدرها من هنا.",
    hint_stats: "ساعات المشاهدة والصفحات المقروءة وتوزيع تقييماتك.",
    hint_calendar: "جداول العرض وتواريخ الإصدار القادمة.",
    hint_settings: "تفضيلات السمة واللغة والبيانات والموزِّعين.",
  },
  page: {
    status_bar: "شريط الحالة",
  },
  settings: {
    theme: "السمة",
    themeHint: "فاتح، داكن، أو تتبُّع مظهر النظام.",
    language: "اللغة",
    languageHint: "لغة الواجهة. العربية تُحوِّل التخطيط إلى RTL.",
  },
};

export const resources = { en: { translation: en }, ar: { translation: ar } } as const;

export type AppLanguage = keyof typeof resources;
