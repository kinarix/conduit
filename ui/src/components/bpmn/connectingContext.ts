import { createContext, useContext } from 'react';

export const ConnectingContext = createContext(false);
export const useIsConnecting = () => useContext(ConnectingContext);
