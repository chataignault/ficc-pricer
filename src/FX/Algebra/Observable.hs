{-# LANGUAGE GADTs #-}

module FX.Algebra.Observable
  ( Observable(..)
  , Direction(..)
  , Level
  ) where

import Data.Time (Day)

-- | Direction for barriers
data Direction = Up | Down
  deriving (Show, Eq)

-- | Type alias for barrier level
type Level = Double

-- | Observable values that can be evaluated from market state
data Observable a where
  -- | Constant observable
  Const     :: a -> Observable a

  -- | Current spot rate between two currencies
  SpotRate  :: String -> String -> Observable Double

  -- | Forward rate at a future date
  FwdRate   :: String -> String -> Day -> Observable Double

  -- | Barrier condition (check if observable crosses level)
  Barrier   :: Direction -> Level -> Observable Double -> Observable Bool

  -- | Map function over observable
  Map       :: (a -> b) -> Observable a -> Observable b

  -- | Apply function observable to value observable
  Apply     :: Observable (a -> b) -> Observable a -> Observable b

deriving instance Show a => Show (Observable a)

-- | Functor instance for Observable
instance Functor Observable where
  fmap = Map

-- | Applicative instance for Observable
instance Applicative Observable where
  pure = Const
  (<*>) = Apply

-- | Numeric instance for Observable Double
instance Num a => Num (Observable a) where
  (+) = liftA2 (+)
  (*) = liftA2 (*)
  (-) = liftA2 (-)
  negate = fmap negate
  abs = fmap abs
  signum = fmap signum
  fromInteger = pure . fromInteger

-- | Fractional instance for Observable Double
instance Fractional a => Fractional (Observable a) where
  (/) = liftA2 (/)
  fromRational = pure . fromRational

-- | Helper for lifting binary operations
liftA2 :: Applicative f => (a -> b -> c) -> f a -> f b -> f c
liftA2 f a b = f <$> a <*> b