import { useCallback } from "react";
import { useTranslation } from "react-i18next";

/* A11y baseline (MISSION-037) — first tab stop in the shell: a skip-to-content
   link that slides into view on keyboard focus and moves focus to the content
   landmark when activated. */

export function SkipLink() {
  const { t } = useTranslation();

  const skip = useCallback((event: React.MouseEvent<HTMLAnchorElement>) => {
    const target = document.getElementById("main-content");
    if (!target) return;
    event.preventDefault();
    target.focus();
  }, []);

  return (
    <a href="#main-content" className="skip-link" onClick={skip}>
      {t("a11y.skipToContent")}
    </a>
  );
}
