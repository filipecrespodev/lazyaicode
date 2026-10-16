import { useEffect, useRef } from "react";
import { getGlobalGitIdentity } from "./api";
import { useProfiles } from "./useProfiles";
import { t } from "../../../lib/i18n/react";

/** Seeds a "Default" profile from the machine's global git identity the
 * first time the app runs. Called once from the app root — the underlying
 * profiles list is shared app-wide via the query cache, so this must not
 * run from every `useProfiles()` call site or concurrent mounts could each
 * see an empty list and create duplicate "Default" profiles. */
export function useEnsureDefaultProfile() {
  const { profilesLoaded, profiles, addProfile } = useProfiles();
  const attempted = useRef(false);

  useEffect(() => {
    if (!profilesLoaded || profiles.length > 0 || attempted.current) return;
    attempted.current = true;
    getGlobalGitIdentity().then((identity) => {
      addProfile({ label: t("gitProfiles.defaultProfile"), name: identity.name ?? "", email: identity.email ?? "" });
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [profilesLoaded, profiles.length]);
}
