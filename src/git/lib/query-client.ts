import { MutationCache, QueryClient } from "@tanstack/react-query";

export const queryClient: QueryClient = new QueryClient({
  // Every git action is recorded by the backend's undo history, so after any
  // mutation the Undo/Redo buttons need to re-read what's on top of it.
  mutationCache: new MutationCache({
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ["undoStatus"] });
      // Same for the reflog: every ref-moving action adds an entry to it.
      queryClient.invalidateQueries({ queryKey: ["reflog"] });
    },
  }),
});
