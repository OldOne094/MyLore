/** Tiny class-name joiner used by the primitives (filter empty tokens). */
export function cn(...classes: Array<string | false | null | undefined>): string {
  return classes.filter(Boolean).join(" ");
}
