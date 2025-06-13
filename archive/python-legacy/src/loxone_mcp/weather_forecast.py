"""Weather forecast integration for Loxone MCP server.

Provides daily and hourly weather forecasts using external weather APIs.

SPDX-License-Identifier: MIT
Copyright (c) 2025 Ralf Anton Beier
"""

import logging
from datetime import datetime
from typing import Any, Literal

import httpx

logger = logging.getLogger(__name__)

# Weather API providers - you can choose one
WEATHER_PROVIDERS = {
    "openweathermap": {
        "base_url": "https://api.openweathermap.org/data/2.5",
        "requires_api_key": True,
        "supports_hourly": True,
        "supports_daily": True,
    },
    "open-meteo": {
        "base_url": "https://api.open-meteo.com/v1",
        "requires_api_key": False,
        "supports_hourly": True,
        "supports_daily": True,
    },
}


class WeatherForecastClient:
    """Client for fetching weather forecasts."""

    def __init__(
        self,
        provider: Literal["openweathermap", "open-meteo"] = "open-meteo",
        api_key: str | None = None,
        latitude: float | None = None,
        longitude: float | None = None,
    ) -> None:
        """
        Initialize weather forecast client.

        Args:
            provider: Weather API provider to use
            api_key: API key (required for some providers)
            latitude: Location latitude
            longitude: Location longitude
        """
        self.provider = provider
        self.api_key = api_key
        self.latitude = latitude
        self.longitude = longitude
        self.client = httpx.AsyncClient(timeout=30.0)

    async def close(self) -> None:
        """Close the HTTP client."""
        await self.client.aclose()

    async def get_location_from_loxone(
        self, loxone_structure: dict[str, Any]
    ) -> tuple[float, float]:
        """
        Extract location from Loxone structure file.

        Args:
            loxone_structure: Loxone structure data

        Returns:
            Tuple of (latitude, longitude)
        """
        # Try to find location in Loxone config
        server_info = loxone_structure.get("serverInfo", {})
        location = server_info.get("location", {})

        lat = location.get("latitude")
        lon = location.get("longitude")

        if lat and lon:
            return float(lat), float(lon)

        # Fallback - you should configure this
        logger.warning("No location found in Loxone config, using default")
        return 48.1351, 11.5820  # Munich, Germany as default

    async def get_daily_forecast(self, days: int = 7) -> dict[str, Any]:
        """
        Get daily weather forecast.

        Args:
            days: Number of days to forecast (default 7)

        Returns:
            Daily forecast data
        """
        if not self.latitude or not self.longitude:
            return {"error": "Location not configured"}

        if self.provider == "open-meteo":
            return await self._get_open_meteo_daily(days)
        elif self.provider == "openweathermap":
            return await self._get_openweathermap_daily(days)
        else:
            return {"error": f"Unknown provider: {self.provider}"}

    async def get_hourly_forecast(self, hours: int = 48) -> dict[str, Any]:
        """
        Get hourly weather forecast.

        Args:
            hours: Number of hours to forecast (default 48)

        Returns:
            Hourly forecast data
        """
        if not self.latitude or not self.longitude:
            return {"error": "Location not configured"}

        if self.provider == "open-meteo":
            return await self._get_open_meteo_hourly(hours)
        elif self.provider == "openweathermap":
            return await self._get_openweathermap_hourly(hours)
        else:
            return {"error": f"Unknown provider: {self.provider}"}

    async def _get_open_meteo_daily(self, days: int) -> dict[str, Any]:
        """Get daily forecast from Open-Meteo (free, no API key required)."""
        try:
            url = f"{WEATHER_PROVIDERS['open-meteo']['base_url']}/forecast"
            params = {
                "latitude": self.latitude,
                "longitude": self.longitude,
                "daily": [
                    "temperature_2m_max",
                    "temperature_2m_min",
                    "precipitation_sum",
                    "precipitation_probability_max",
                    "windspeed_10m_max",
                    "weathercode",
                ],
                "timezone": "auto",
                "forecast_days": min(days, 16),  # Open-Meteo supports up to 16 days
            }

            response = await self.client.get(url, params=params)
            response.raise_for_status()
            data = response.json()

            # Transform to consistent format
            daily_data = data.get("daily", {})
            forecasts = []

            for i in range(len(daily_data.get("time", []))):
                forecasts.append(
                    {
                        "date": daily_data["time"][i],
                        "temperature_max": daily_data["temperature_2m_max"][i],
                        "temperature_min": daily_data["temperature_2m_min"][i],
                        "precipitation": daily_data["precipitation_sum"][i],
                        "precipitation_probability": daily_data["precipitation_probability_max"][i],
                        "wind_speed": daily_data["windspeed_10m_max"][i],
                        "weather_code": daily_data["weathercode"][i],
                        "description": self._get_weather_description(daily_data["weathercode"][i]),
                    }
                )

            return {
                "provider": "open-meteo",
                "location": {"latitude": self.latitude, "longitude": self.longitude},
                "daily": forecasts,
                "updated": datetime.now().isoformat(),
            }

        except Exception as e:
            logger.error(f"Failed to get Open-Meteo daily forecast: {e}")
            return {"error": f"Failed to get forecast: {e}"}

    async def _get_open_meteo_hourly(self, hours: int) -> dict[str, Any]:
        """Get hourly forecast from Open-Meteo."""
        try:
            url = f"{WEATHER_PROVIDERS['open-meteo']['base_url']}/forecast"
            params = {
                "latitude": self.latitude,
                "longitude": self.longitude,
                "hourly": [
                    "temperature_2m",
                    "precipitation",
                    "precipitation_probability",
                    "windspeed_10m",
                    "weathercode",
                    "relativehumidity_2m",
                ],
                "timezone": "auto",
                "forecast_hours": min(hours, 384),  # Open-Meteo supports up to 384 hours
            }

            response = await self.client.get(url, params=params)
            response.raise_for_status()
            data = response.json()

            # Transform to consistent format
            hourly_data = data.get("hourly", {})
            forecasts = []

            for i in range(min(len(hourly_data.get("time", [])), hours)):
                forecasts.append(
                    {
                        "time": hourly_data["time"][i],
                        "temperature": hourly_data["temperature_2m"][i],
                        "precipitation": hourly_data["precipitation"][i],
                        "precipitation_probability": hourly_data["precipitation_probability"][i],
                        "wind_speed": hourly_data["windspeed_10m"][i],
                        "humidity": hourly_data["relativehumidity_2m"][i],
                        "weather_code": hourly_data["weathercode"][i],
                        "description": self._get_weather_description(hourly_data["weathercode"][i]),
                    }
                )

            return {
                "provider": "open-meteo",
                "location": {"latitude": self.latitude, "longitude": self.longitude},
                "hourly": forecasts,
                "updated": datetime.now().isoformat(),
            }

        except Exception as e:
            logger.error(f"Failed to get Open-Meteo hourly forecast: {e}")
            return {"error": f"Failed to get forecast: {e}"}

    async def _get_openweathermap_daily(self, days: int) -> dict[str, Any]:
        """Get daily forecast from OpenWeatherMap (requires API key)."""
        if not self.api_key:
            return {"error": "OpenWeatherMap requires API key"}

        try:
            url = f"{WEATHER_PROVIDERS['openweathermap']['base_url']}/forecast/daily"
            params = {
                "lat": self.latitude,
                "lon": self.longitude,
                "cnt": min(days, 16),
                "appid": self.api_key,
                "units": "metric",
            }

            response = await self.client.get(url, params=params)
            response.raise_for_status()
            data = response.json()

            forecasts = []
            for day in data.get("list", []):
                dt = datetime.fromtimestamp(day["dt"])
                forecasts.append(
                    {
                        "date": dt.strftime("%Y-%m-%d"),
                        "temperature_max": day["temp"]["max"],
                        "temperature_min": day["temp"]["min"],
                        "precipitation": day.get("rain", 0),
                        "precipitation_probability": day.get("pop", 0) * 100,
                        "wind_speed": day["speed"],
                        "weather_code": day["weather"][0]["id"],
                        "description": day["weather"][0]["description"],
                    }
                )

            return {
                "provider": "openweathermap",
                "location": {
                    "latitude": self.latitude,
                    "longitude": self.longitude,
                    "city": data.get("city", {}).get("name", "Unknown"),
                },
                "daily": forecasts,
                "updated": datetime.now().isoformat(),
            }

        except Exception as e:
            logger.error(f"Failed to get OpenWeatherMap daily forecast: {e}")
            return {"error": f"Failed to get forecast: {e}"}

    async def _get_openweathermap_hourly(self, hours: int) -> dict[str, Any]:
        """Get hourly forecast from OpenWeatherMap."""
        if not self.api_key:
            return {"error": "OpenWeatherMap requires API key"}

        try:
            # OpenWeatherMap free tier only provides 5-day/3-hour forecast
            url = f"{WEATHER_PROVIDERS['openweathermap']['base_url']}/forecast"
            params = {
                "lat": self.latitude,
                "lon": self.longitude,
                "appid": self.api_key,
                "units": "metric",
            }

            response = await self.client.get(url, params=params)
            response.raise_for_status()
            data = response.json()

            forecasts = []
            for item in data.get("list", [])[: hours // 3]:  # 3-hour intervals
                dt = datetime.fromtimestamp(item["dt"])
                forecasts.append(
                    {
                        "time": dt.isoformat(),
                        "temperature": item["main"]["temp"],
                        "precipitation": item.get("rain", {}).get("3h", 0),
                        "precipitation_probability": item.get("pop", 0) * 100,
                        "wind_speed": item["wind"]["speed"],
                        "humidity": item["main"]["humidity"],
                        "weather_code": item["weather"][0]["id"],
                        "description": item["weather"][0]["description"],
                    }
                )

            return {
                "provider": "openweathermap",
                "location": {
                    "latitude": self.latitude,
                    "longitude": self.longitude,
                    "city": data.get("city", {}).get("name", "Unknown"),
                },
                "hourly": forecasts,
                "updated": datetime.now().isoformat(),
                "note": "OpenWeatherMap free tier provides 3-hour intervals",
            }

        except Exception as e:
            logger.error(f"Failed to get OpenWeatherMap hourly forecast: {e}")
            return {"error": f"Failed to get forecast: {e}"}

    def _get_weather_description(self, code: int) -> str:
        """Convert weather code to description."""
        # WMO Weather interpretation codes (used by Open-Meteo)
        weather_codes = {
            0: "Clear sky",
            1: "Mainly clear",
            2: "Partly cloudy",
            3: "Overcast",
            45: "Foggy",
            48: "Depositing rime fog",
            51: "Light drizzle",
            53: "Moderate drizzle",
            55: "Dense drizzle",
            61: "Slight rain",
            63: "Moderate rain",
            65: "Heavy rain",
            71: "Slight snow",
            73: "Moderate snow",
            75: "Heavy snow",
            77: "Snow grains",
            80: "Slight rain showers",
            81: "Moderate rain showers",
            82: "Violent rain showers",
            85: "Slight snow showers",
            86: "Heavy snow showers",
            95: "Thunderstorm",
            96: "Thunderstorm with slight hail",
            99: "Thunderstorm with heavy hail",
        }
        return weather_codes.get(code, f"Unknown ({code})")
