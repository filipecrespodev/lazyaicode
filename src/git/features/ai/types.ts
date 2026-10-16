export type AiModel = {
  id: string;
  label: string;
};

export type AiSettings = {
  commitMessageModel: string;
  commitMessageCustomInstructions: string;
  composeCustomInstructions: string;
  conflictCustomInstructions: string;
};
