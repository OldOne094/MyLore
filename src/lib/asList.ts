/* IPC list payloads are foreign data: the generated wrappers *claim* an array,
   but a reply of the wrong shape used to reach `.map` and take a whole page down
   mid-render — a suite that passes while carrying an unhandled error.

   Normalise at the query boundary so no consumer has to remember the difference
   between "no data yet" and "the wrong shape". The parameter type mirrors the
   contract we expect (`T[]`, so callers keep full inference); the runtime check
   is what makes a lying backend harmless. */

export function asList<T>(value: T[]): T[] {
  return Array.isArray(value) ? value : [];
}
