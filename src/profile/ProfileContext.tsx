import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from "react";
import { DEFAULT_PROFILE, type UserProfile } from "./profile";

const PROFILE_KEY = "mylore.profile";

interface ProfileContextValue {
  profile: UserProfile;
  setProfile: (profile: Partial<UserProfile>) => void;
}

const ProfileContext = createContext<ProfileContextValue | null>(null);

function loadProfile(): UserProfile {
  try {
    const raw = localStorage.getItem(PROFILE_KEY);
    if (!raw) return DEFAULT_PROFILE;
    const parsed = JSON.parse(raw);
    return {
      displayName: typeof parsed.displayName === "string" ? parsed.displayName : "",
      avatarColor:
        typeof parsed.avatarColor === "string" ? parsed.avatarColor : DEFAULT_PROFILE.avatarColor,
    };
  } catch {
    return DEFAULT_PROFILE;
  }
}

export function ProfileProvider({ children }: { children: ReactNode }) {
  const [profile, setProfileState] = useState<UserProfile>(loadProfile);

  const setProfile = useCallback((update: Partial<UserProfile>) => {
    setProfileState((current) => {
      const next = { ...current, ...update };
      try {
        localStorage.setItem(PROFILE_KEY, JSON.stringify(next));
      } catch {
        /* storage unavailable */
      }
      return next;
    });
  }, []);

  const value = useMemo(() => ({ profile, setProfile }), [profile, setProfile]);

  return <ProfileContext.Provider value={value}>{children}</ProfileContext.Provider>;
}

// eslint-disable-next-line react-refresh/only-export-components
export function useProfile(): ProfileContextValue {
  const ctx = useContext(ProfileContext);
  // Graceful fallback for test environments without a provider.
  if (!ctx) return { profile: DEFAULT_PROFILE, setProfile: () => {} };
  return ctx;
}
