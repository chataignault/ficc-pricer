module FX.Pricing.MarketData
  ( MarketState(..)
  , PricingParams(..)
  , PricingModel(..)
  , emptyMarketState
  ) where

import Data.Map.Strict (Map)
import qualified Data.Map.Strict as Map
import Data.Time (Day)
import FX.Algebra.Contract (Currency, Strike, Date)

-- | Market state containing all observable market data
data MarketState = MarketState
  { spotRates       :: Map (Currency, Currency) Double
    -- ^ Current spot FX rates
  , discountCurves  :: Map Currency (Date -> Double)
    -- ^ Discount factor curves (date -> discount factor)
  , volSurfaces     :: Map (Currency, Currency) (Strike -> Date -> Double)
    -- ^ Volatility surfaces for FX pairs
  , correlations    :: Map (Currency, Currency, Currency, Currency) Double
    -- ^ Correlation matrix for multi-asset products
  }

-- | Pricing parameters
data PricingParams = PricingParams
  { valuationDate   :: Date
    -- ^ Date for valuation
  , numeraire       :: Currency
    -- ^ Currency for expressing all values
  , model           :: PricingModel
    -- ^ Pricing model to use
  }
  deriving (Show, Eq)

-- | Available pricing models
data PricingModel
  = BlackScholes
  | LocalVol
  | Heston
  deriving (Show, Eq)

-- | Create an empty market state
emptyMarketState :: MarketState
emptyMarketState = MarketState
  { spotRates = Map.empty
  , discountCurves = Map.empty
  , volSurfaces = Map.empty
  , correlations = Map.empty
  }