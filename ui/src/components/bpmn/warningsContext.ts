import { createContext, useContext } from 'react';

export const WarningsContext = createContext<Record<string, string[]>>({});

export const useNodeWarnings = (id: string): string[] =>
  useContext(WarningsContext)[id] ?? [];
