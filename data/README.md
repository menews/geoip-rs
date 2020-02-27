### maxmind db

Go to https://dev.maxmind.com/geoip/geoip2/geolite2/#Downloads, download 'GeoLite2 City' binary db, unpack the zip in this folder


### Arabic country names

to return arabic name country `country_name_ar` in the response you need to create a josn file contains the localized country names as the following example:

    {
      "AE": "الامارات العربية المتحدة",
      .
      .
      .
    }
    
then set the path to the file in `.env` file
    
    GEOIP_RS_COUNTRY_NAMES_AR=data/countries_ar.json

