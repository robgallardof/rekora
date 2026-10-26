class OrderRepository:
    """Stores orders in memory."""

    def __init__(self):
        self._orders = {}

    def save(self, order_id, order):
        self._orders[order_id] = order


def load_orders(repo):
    return list(repo._orders.values())


# Dynamic behaviour: this attribute exists only at runtime and must not be
# reported as a declared symbol.
setattr(OrderRepository, "export_all", lambda self: dict(self._orders))
