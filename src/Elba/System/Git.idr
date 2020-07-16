module Elba.System.Git

import System
import System.Path

-- TODO: An error type
-- Maybe follow Idris compiler pattern: a "Core" monad which is a type syn for IO (Either Error _)
  
record GitSpec where
  constructor MkGit
  url, branch : String

clone : GitSpec -> Path -> IO ()
clone s p = do
  system $ "git clone " ++ s.url ++ " " ++ show p
  pure ()
