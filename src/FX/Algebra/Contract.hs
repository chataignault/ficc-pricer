{-# LANGUAGE GADTs #-}

module FX.Algebra.Contract
  ( Contract(..)
  , Currency(..)
  , OptionType(..)
  , Date
  , Rate
  , Strike
  ) where

import Data.Time (Day)
import FX.Algebra.Observable (Observable)

-- | Supported currencies
data Currency = USD | EUR | GBP | JPY | CHF | AUD | CAD
  deriving (Show, Read, Eq, Ord)

-- | Option type
data OptionType = Call | Put
  deriving (Show, Eq)

-- | Type aliases for clarity
type Date = Day
type Rate = Double
type Strike = Double

-- | Core contract algebra using GADTs
data Contract where
  -- | Identity contract (zero value)
  Zero      :: Contract

  -- | Immediate FX spot exchange
  Spot      :: Currency -> Currency -> Contract

  -- | Forward FX contract at a fixed rate
  Forward   :: Date -> Rate -> Currency -> Currency -> Contract

  -- | European FX option
  EurOption :: OptionType -> Strike -> Date -> Currency -> Currency -> Contract

  -- | Zero coupon bond (pays 1 unit of currency at maturity)
  ZCB       :: Currency -> Date -> Contract

  -- | Scale a contract by a notional amount
  Scale     :: Double -> Contract -> Contract

  -- | Combine two contracts into a portfolio
  Combine   :: Contract -> Contract -> Contract

  -- | Conditional execution based on an observable
  When      :: Observable Bool -> Contract -> Contract

instance Show Contract where
  show Zero = "Zero"
  show (Spot c1 c2) = "Spot " ++ show c1 ++ " " ++ show c2
  show (Forward d r c1 c2) = "Forward " ++ show d ++ " " ++ show r ++ " " ++ show c1 ++ " " ++ show c2
  show (EurOption ot k d c1 c2) = "EurOption " ++ show ot ++ " " ++ show k ++ " " ++ show d ++ " " ++ show c1 ++ " " ++ show c2
  show (ZCB c d) = "ZCB " ++ show c ++ " " ++ show d
  show (Scale n c) = "Scale " ++ show n ++ " (" ++ show c ++ ")"
  show (Combine c1 c2) = "Combine (" ++ show c1 ++ ") (" ++ show c2 ++ ")"
  show (When _ c) = "When <obs> (" ++ show c ++ ")"

-- | Monoid instance for portfolio combination
-- Smart constructor that eliminates Zero (but doesn't flatten for structural preservation)
instance Semigroup Contract where
  Zero <> c = c
  c <> Zero = c
  Scale _ Zero <> c = c
  c <> Scale _ Zero = c
  c1 <> c2 = Combine c1 c2

instance Monoid Contract where
  mempty = Zero