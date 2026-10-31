import React from 'react';
import { createRoot } from 'react-dom/client';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { App } from './ui/App';
import { ConfirmationProvider } from './ui/ConfirmationProvider';
import { IslandWindow } from './ui/IslandWindow';
import '@fontsource-variable/inter';
import './ui/design-tokens.css';
import './ui/styles.css';
import './ui/settings-controls.css';

const label = isTauri() ? getCurrentWindow().label : 'main';
const view =
  label === 'pet' ? (
    <IslandWindow />
  ) : (
    <ConfirmationProvider>
      <App />
    </ConfirmationProvider>
  );

createRoot(document.getElementById('root')!).render(<React.StrictMode>{view}</React.StrictMode>);
