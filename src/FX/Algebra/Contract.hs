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
  deriving (Show, Eq, Ord)

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

deriving instance Show Contract

-- | Monoid instance for portfolio combination
instance Semigroup Contract where
  (<>) = Combine

instance Monoid Contract where
  mempty = Zero