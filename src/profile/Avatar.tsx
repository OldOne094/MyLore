import { cn } from "@/lib/cn";
import { initials } from "./profile";

/* MISSION-132 — Avatar circle with initials and a solid accent color.
   Used in the nav rail, shareable stats card and settings profile editor. */

export function Avatar({
  displayName,
  color,
  size = 36,
  className,
}: {
  displayName: string;
  color: string;
  size?: number;
  className?: string;
}) {
  return (
    <div
      aria-hidden="true"
      style={{ width: size, height: size, backgroundColor: color, fontSize: size * 0.38 }}
      className={cn(
        "flex shrink-0 select-none items-center justify-center rounded-full font-semibold text-white",
        className,
      )}
    >
      {initials(displayName)}
    </div>
  );
}
