import { useState } from 'react';
import { SpacetimeProvider } from './hooks/useSpacetimeDB';
import { Lobby } from './components/Lobby';
import { GameTable } from './components/GameTable';
import './App.css';

const SPACETIME_HOST = import.meta.env.VITE_SPACETIME_HOST ?? 'http://localhost:3000';
const SPACETIME_MODULE = import.meta.env.VITE_SPACETIME_MODULE ?? 'kortbridge';

function App() {
  const [currentGameId, setCurrentGameId] = useState<bigint | null>(null);

  return (
    <SpacetimeProvider host={SPACETIME_HOST} moduleName={SPACETIME_MODULE}>
      <div className="app">
        {currentGameId === null ? (
          <Lobby onJoinGame={setCurrentGameId} />
        ) : (
          <GameTable gameId={currentGameId} onLeave={() => setCurrentGameId(null)} />
        )}
      </div>
    </SpacetimeProvider>
  );
}

export default App;
