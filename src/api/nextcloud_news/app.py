"""API"""

from fastapi import APIRouter
from fastapi.security import HTTPBasic

from .v1_2.app import router as router_v1_2

router = APIRouter()

security = HTTPBasic(auto_error=False)

router.include_router(router_v1_2, prefix="/v1-2")
