import type { TFunction } from "i18next";

/* MISSION-061 — label/value helpers for the enrich diff dialog. Field names are
   the backend's snake_case keys; values are either null (no change known) or a
   comma-joined label string from the backend. */

export function prettyFieldLabel(field: string, t: TFunction): string {
  const key = `enrich.field${toPascalCase(field)}`;
  const label = t(key);
  return label === key ? field : label;
}

function toPascalCase(field: string): string {
  return field.replace(/(^|_)(\w)/g, (_, _sep: string, char: string) => char.toUpperCase());
}

export function prettyValue(value: string | null): string {
  return value && value.trim() !== "" ? value : "—";
}
